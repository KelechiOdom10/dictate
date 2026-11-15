//! Pulls still frames out of an MP4 with `AVAssetImageGenerator`.
//!
//! Called through `objc2-av-foundation` rather than a Swift helper: the
//! bindings cover everything needed (asset, generator, zero tolerance, the
//! non-deprecated async call), so there is no second toolchain, no binary to
//! ship and no subprocess on the stop-to-paste path.
//!
//! Every time is requested at once with zero tolerance, so the generator
//! decodes each GOP once and returns the exact frame shown at that time. Each
//! `CGImage` is drawn straight into a tiny-skia pixmap (premultiplied RGBA,
//! sRGB) inside the callback, so no CoreGraphics object crosses threads.

use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2_av_foundation::{AVAsset, AVAssetImageGenerator};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo,
    CGImageByteOrderInfo, kCGColorSpaceSRGB,
};
use objc2_core_media::{CMTime, kCMTimeZero};
use objc2_foundation::{NSError, NSURL};
use tiny_skia::Pixmap;

/// Generous: 12 frames from a 60 s recording take well under a second.
const TIMEOUT: Duration = Duration::from_secs(30);

pub struct Extracted {
    pub pixmap: Pixmap,
    /// The video time of the frame AVFoundation returned.
    pub actual_ms: u64,
}

/// One image per entry of `times_ms` (video time), in the same order.
pub fn frames(video: &Path, times_ms: &[u64]) -> Result<Vec<Extracted>, String> {
    if !video.is_file() {
        return Err(format!("Recording not found at {}", video.display()));
    }
    let url = NSURL::from_file_path(video)
        .ok_or_else(|| format!("Not a file path: {}", video.display()))?;
    // SAFETY: plain AVFoundation calls on objects this function owns.
    let generator = unsafe {
        let asset = AVAsset::assetWithURL(&url);
        let generator = AVAssetImageGenerator::assetImageGeneratorWithAsset(&asset);
        generator.setAppliesPreferredTrackTransform(true);
        generator.setRequestedTimeToleranceBefore(kCMTimeZero);
        generator.setRequestedTimeToleranceAfter(kCMTimeZero);
        generator
    };

    let (sender, receiver) = mpsc::channel();
    for (index, &t) in times_ms.iter().enumerate() {
        let sender = sender.clone();
        let handler = RcBlock::new(
            move |image: *mut CGImage, actual: CMTime, error: *mut NSError| {
                // SAFETY: AVFoundation passes a valid image or null, and a
                // valid error or null, for the duration of the callback.
                let result = match unsafe { image.as_ref() } {
                    Some(image) => pixmap_of(image).map(|pixmap| Extracted {
                        pixmap,
                        // SAFETY: CMTimeGetSeconds is a pure function.
                        actual_ms: (unsafe { actual.seconds() } * 1000.0).round().max(0.0) as u64,
                    }),
                    None => Err(unsafe { error.as_ref() }
                        .map_or("no image and no error".into(), |e| {
                            e.localizedDescription().to_string()
                        })),
                };
                let _ = sender.send((index, result));
            },
        );
        // SAFETY: the block is retained by AVFoundation until it has run.
        unsafe {
            generator.generateCGImageAsynchronouslyForTime_completionHandler(
                CMTime::new(t as i64, 1000),
                &handler,
            );
        }
    }
    drop(sender);

    let deadline = Instant::now() + TIMEOUT;
    let mut out: Vec<Option<Extracted>> = times_ms.iter().map(|_| None).collect();
    for _ in times_ms {
        let wait = deadline.saturating_duration_since(Instant::now());
        let (index, result) = receiver.recv_timeout(wait).map_err(|_| {
            // SAFETY: cancelling is always allowed; pending handlers still run.
            unsafe { generator.cancelAllCGImageGeneration() };
            format!("Frame extraction timed out after {} s", TIMEOUT.as_secs())
        })?;
        let image = result.map_err(|e| {
            format!(
                "Frame at {} ms of {}: {e}",
                times_ms[index],
                video.display()
            )
        })?;
        out[index] = Some(image);
    }
    out.into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| "A frame was not returned".to_string())
}

fn pixmap_of(image: &CGImage) -> Result<Pixmap, String> {
    let width = CGImage::width(Some(image));
    let height = CGImage::height(Some(image));
    let mut pixmap = Pixmap::new(width as u32, height as u32)
        .ok_or_else(|| format!("Bad frame size {width}×{height}"))?;
    // SAFETY: kCGColorSpaceSRGB is an immutable framework constant.
    let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }));
    let info = CGImageAlphaInfo::PremultipliedLast.0 | CGImageByteOrderInfo::Order32Big.0;
    // SAFETY: the buffer is width × height × 4 bytes and outlives the context,
    // which is dropped before the pixmap is returned.
    let context = unsafe {
        CGBitmapContextCreate(
            pixmap.data_mut().as_mut_ptr().cast(),
            width,
            height,
            8,
            width * 4,
            space.as_deref(),
            info,
        )
    }
    .ok_or("Create a bitmap context for a frame")?;
    let rect = CGRect::new(
        CGPoint::new(0.0, 0.0),
        CGSize::new(width as f64, height as f64),
    );
    CGContext::draw_image(Some(&context), rect, Some(image));
    drop(context);
    Ok(pixmap)
}
