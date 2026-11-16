//! A screen-recording stand-in for the Annotate tests: an H.264 MP4 written
//! with AVAssetWriter, so no Screen Recording permission and no ffmpeg are
//! needed. Each video frame carries its own index as a 10-bit barcode across
//! the top, so a test can tell exactly which frame AVFoundation returned.

#![cfg(target_os = "macos")]
#![allow(dead_code)]

use std::path::Path;
use std::ptr::NonNull;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2_av_foundation::{
    AVAssetWriter, AVAssetWriterInput, AVAssetWriterInputPixelBufferAdaptor, AVAssetWriterStatus,
    AVFileTypeMPEG4, AVMediaTypeVideo, AVVideoCodecKey, AVVideoCodecTypeH264, AVVideoHeightKey,
    AVVideoWidthKey,
};
use objc2_core_foundation::CFRetained;
use objc2_core_media::{CMTime, kCMTimeZero};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferCreate, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
    kCVPixelFormatType_32BGRA,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};
use tiny_skia::Pixmap;

pub const WIDTH: usize = 2560;
pub const HEIGHT: usize = 1600;
pub const FPS: u64 = 15;
const BARCODE_BITS: usize = 10;
const BARCODE_ROWS: usize = 80;

pub fn read_barcode(pixmap: &Pixmap) -> u64 {
    let block = pixmap.width() as usize / BARCODE_BITS;
    (0..BARCODE_BITS).fold(0, |value, bit| {
        let x = (bit * block + block / 2) as u32;
        let p = pixmap.pixel(x, (BARCODE_ROWS / 2) as u32).unwrap();
        (value << 1) | u64::from(p.green() > 128)
    })
}

/// One BGRA video frame: a dark desktop with a lighter "window" that moves
/// every second, and the frame index as a barcode across the top.
fn paint_frame(index: u64, pixels: &mut [u8], stride: usize) {
    let second = index / FPS;
    let desktop = [
        60 + (second * 37 % 120) as u8,
        70 + (second * 23 % 60) as u8,
        40 + (second * 11 % 50) as u8,
        255,
    ];
    let window = [210, 205, 200, 255];
    let title = [170, 165, 160, 255];
    let left = 300 + (second as usize * 40) % 600;
    let (right, top, bottom) = (left + 1500, 220, 1400);

    let mut desktop_row = Vec::with_capacity(WIDTH * 4);
    let mut window_row = Vec::with_capacity(WIDTH * 4);
    let mut title_row = Vec::with_capacity(WIDTH * 4);
    for x in 0..WIDTH {
        let inside = (left..right).contains(&x);
        desktop_row.extend_from_slice(&desktop);
        window_row.extend_from_slice(if inside { &window } else { &desktop });
        title_row.extend_from_slice(if inside { &title } else { &desktop });
    }
    let block = WIDTH / BARCODE_BITS;
    let barcode_row: Vec<u8> = (0..WIDTH)
        .flat_map(|x| {
            let bit = BARCODE_BITS - 1 - (x / block).min(BARCODE_BITS - 1);
            let on = (index >> bit) & 1 == 1;
            if on {
                [255, 255, 255, 255]
            } else {
                [0, 0, 0, 255]
            }
        })
        .collect();
    for y in 0..HEIGHT {
        let row = match y {
            _ if y < BARCODE_ROWS => &barcode_row,
            _ if (top..top + 60).contains(&y) => &title_row,
            _ if (top..bottom).contains(&y) => &window_row,
            _ => &desktop_row,
        };
        pixels[y * stride..y * stride + WIDTH * 4].copy_from_slice(row);
    }
}

pub fn write_test_video(path: &Path, seconds: u64) -> Result<(), String> {
    let _ = std::fs::remove_file(path);
    let url = NSURL::from_file_path(path).ok_or("bad path")?;
    // SAFETY: AVFoundation calls on objects this function owns; the statics
    // are framework constants.
    unsafe {
        let file_type = AVFileTypeMPEG4.ok_or("AVFileTypeMPEG4 missing")?;
        let writer = AVAssetWriter::assetWriterWithURL_fileType_error(&url, file_type)
            .map_err(|e| e.localizedDescription().to_string())?;
        let codec_key = AVVideoCodecKey.ok_or("AVVideoCodecKey missing")?;
        let width_key = AVVideoWidthKey.ok_or("AVVideoWidthKey missing")?;
        let height_key = AVVideoHeightKey.ok_or("AVVideoHeightKey missing")?;
        let codec: &NSString = AVVideoCodecTypeH264.ok_or("H.264 missing")?;
        let width = NSNumber::new_u32(WIDTH as u32);
        let height = NSNumber::new_u32(HEIGHT as u32);
        let settings = NSDictionary::from_slices(
            &[codec_key, width_key, height_key],
            &[&***codec, &***width, &***height],
        );
        let input = AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(
            AVMediaTypeVideo.ok_or("AVMediaTypeVideo missing")?,
            Some(&settings),
        );
        input.setExpectsMediaDataInRealTime(false);
        let adaptor = AVAssetWriterInputPixelBufferAdaptor::assetWriterInputPixelBufferAdaptorWithAssetWriterInput_sourcePixelBufferAttributes(&input, None);
        if !writer.canAddInput(&input) {
            return Err("writer refused the video input".into());
        }
        writer.addInput(&input);
        if !writer.startWriting() {
            return Err(format!("startWriting failed: {:?}", writer.error()));
        }
        writer.startSessionAtSourceTime(kCMTimeZero);

        for index in 0..seconds * FPS {
            let waited = Instant::now();
            while !input.isReadyForMoreMediaData() {
                if waited.elapsed() > Duration::from_secs(10) {
                    return Err("writer input stalled".into());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let buffer = pixel_buffer(index)?;
            if !adaptor.appendPixelBuffer_withPresentationTime(
                &buffer,
                CMTime::new(index as i64, FPS as i32),
            ) {
                return Err(format!("append frame {index} failed: {:?}", writer.error()));
            }
        }
        input.markAsFinished();
        let (done, finished) = mpsc::channel();
        let handler = RcBlock::new(move || {
            let _ = done.send(());
        });
        writer.finishWritingWithCompletionHandler(&handler);
        finished
            .recv_timeout(Duration::from_secs(60))
            .map_err(|_| "finishWriting timed out")?;
        if writer.status() != AVAssetWriterStatus::Completed {
            return Err(format!("writer ended with {:?}", writer.error()));
        }
    }
    Ok(())
}

fn pixel_buffer(index: u64) -> Result<CFRetained<CVPixelBuffer>, String> {
    let mut raw: *mut CVPixelBuffer = std::ptr::null_mut();
    // SAFETY: `raw` is a valid out pointer; on success it holds a +1 buffer,
    // which CFRetained takes over. The base address is only used while locked.
    unsafe {
        let status = CVPixelBufferCreate(
            None,
            WIDTH,
            HEIGHT,
            kCVPixelFormatType_32BGRA,
            None,
            NonNull::from(&mut raw),
        );
        let raw = NonNull::new(raw).filter(|_| status == 0);
        let buffer = CFRetained::from_raw(raw.ok_or(format!("CVPixelBufferCreate: {status}"))?);
        CVPixelBufferLockBaseAddress(&buffer, CVPixelBufferLockFlags(0));
        let stride = CVPixelBufferGetBytesPerRow(&buffer);
        let base = CVPixelBufferGetBaseAddress(&buffer).cast::<u8>();
        let pixels = std::slice::from_raw_parts_mut(base, stride * HEIGHT);
        paint_frame(index, pixels, stride);
        CVPixelBufferUnlockBaseAddress(&buffer, CVPixelBufferLockFlags(0));
        Ok(buffer)
    }
}
