//! Record the main display with the `screen` stage alone, then print what
//! came out.
//!
//!   cargo run --example screen_capture -- [--seconds 30] [--fps 15]
//!       [--overlay-at 3] [--include-own-windows]
//!
//! `--overlay-at S` puts a red, click-through, non-activating window in the
//! middle of the display S seconds into the recording, so the MP4 can be
//! checked for our own windows (risk R5). It never takes focus or input.

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("screen_capture needs macOS 15 or later");
}

#[cfg(target_os = "macos")]
fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

#[cfg(target_os = "macos")]
struct Args {
    seconds: f64,
    fps: u32,
    overlay_at: Option<f64>,
    include_own_windows: bool,
}

#[cfg(target_os = "macos")]
fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        seconds: 30.0,
        fps: dictate_lib::screen::DEFAULT_FPS,
        overlay_at: None,
        include_own_windows: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .ok_or_else(|| format!("{name} needs a value"))
                .and_then(|v| v.parse::<f64>().map_err(|e| format!("{name} {v}: {e}")))
        };
        match flag.as_str() {
            "--seconds" => args.seconds = value("--seconds")?,
            "--fps" => args.fps = value("--fps")? as u32,
            "--overlay-at" => args.overlay_at = Some(value("--overlay-at")?),
            "--include-own-windows" => args.include_own_windows = true,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(args)
}

#[cfg(target_os = "macos")]
fn run() -> Result<(), String> {
    use dictate_lib::screen::{self, CaptureOptions, DisplayTarget};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    use std::time::{Duration, Instant};

    let args = parse_args()?;
    let mtm = MainThreadMarker::new().ok_or("must run on the main thread")?;
    // Accessory: no Dock icon, and showing a window never activates us.
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    // The real app always owns a window (its hidden main window), which is
    // what lets ScreenCaptureKit list it as an application to exclude. Mirror
    // that, so the red window below tests a window created after start.
    let _hidden_main_window = test_overlay::hidden(mtm);

    println!("permission preflight: {}", screen::has_permission());
    let options = CaptureOptions {
        fps: args.fps,
        include_own_windows: args.include_own_windows,
    };
    let start_called = Instant::now();
    screen::start(DisplayTarget::Main, options)?;
    let started = Instant::now();
    println!(
        "start() took {} ms; recording {} s at {} fps",
        (started - start_called).as_millis(),
        args.seconds,
        args.fps
    );

    let mut overlay = None;
    let end = started + Duration::from_secs_f64(args.seconds);
    while Instant::now() < end {
        if overlay.is_none()
            && args
                .overlay_at
                .is_some_and(|at| started.elapsed().as_secs_f64() >= at)
        {
            overlay = Some(test_overlay::show(mtm));
            println!("overlay shown at {:.2} s", started.elapsed().as_secs_f64());
        }
        test_overlay::pump_run_loop(Duration::from_millis(20));
    }

    let stop_called = Instant::now();
    let info = screen::stop()?;
    println!("stop() took {} ms", stop_called.elapsed().as_millis());
    if let Some(window) = overlay {
        window.close();
    }

    let expected = info.wall_ms * u64::from(info.fps) / 1000;
    println!("path: {}", info.path.display());
    println!(
        "display {} at {}x{}, {} fps",
        info.display_id, info.width, info.height, info.fps
    );
    println!(
        "duration: {} ms (wall {} ms)",
        info.duration_ms, info.wall_ms
    );
    println!(
        "size: {} bytes ({:.2} MB, {:.2} Mbit/s)",
        info.file_bytes,
        info.file_bytes as f64 / 1e6,
        info.file_bytes as f64 * 8.0 / 1e6 / (info.duration_ms.max(1) as f64 / 1000.0)
    );
    println!(
        "frames delivered: {} (complete {}, idle {}, other {}); {} expected at {} fps",
        info.frames.delivered(),
        info.frames.complete,
        info.frames.idle,
        info.frames.other,
        expected,
        info.fps
    );
    if info.own_windows_excluded {
        println!("own windows excluded: yes");
    } else if args.include_own_windows {
        println!("own windows excluded: no (--include-own-windows)");
    } else {
        println!("own windows excluded: NO, this process was not in the shareable content");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
mod test_overlay {
    use objc2::rc::Retained;
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSBackingStoreType, NSColor, NSScreen, NSStatusWindowLevel, NSWindow, NSWindowStyleMask,
    };
    use objc2_foundation::{NSDate, NSPoint, NSRect, NSRunLoop, NSSize};
    use std::time::Duration;

    pub const SIZE: f64 = 400.0;

    pub fn hidden(mtm: MainThreadMarker) -> Retained<NSWindow> {
        let window = window(
            mtm,
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0)),
        );
        window.orderOut(None);
        window
    }

    /// A red square in the middle of the main screen. Click-through, never
    /// key, never main, so it cannot take focus or input from other apps.
    pub fn show(mtm: MainThreadMarker) -> Retained<NSWindow> {
        let screen = NSScreen::mainScreen(mtm).expect("a main screen");
        let frame = screen.frame();
        let rect = NSRect::new(
            NSPoint::new(
                frame.origin.x + (frame.size.width - SIZE) / 2.0,
                frame.origin.y + (frame.size.height - SIZE) / 2.0,
            ),
            NSSize::new(SIZE, SIZE),
        );
        let window = window(mtm, rect);
        window.setBackgroundColor(Some(&NSColor::redColor()));
        window.setOpaque(true);
        window.setLevel(NSStatusWindowLevel);
        window.orderFrontRegardless();
        window
    }

    fn window(mtm: MainThreadMarker, rect: NSRect) -> Retained<NSWindow> {
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe { window.setReleasedWhenClosed(false) };
        window.setIgnoresMouseEvents(true);
        window
    }

    pub fn pump_run_loop(for_how_long: Duration) {
        let until = NSDate::dateWithTimeIntervalSinceNow(for_how_long.as_secs_f64());
        NSRunLoop::currentRunLoop().runUntilDate(&until);
    }
}
