use crate::settings::OverlayPosition;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition};

static VISIBILITY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn show_is_current(expected: &crate::surface::Status, current: &crate::surface::Status) -> bool {
    if expected.error.is_some() {
        expected.error == current.error
    } else {
        expected == current
    }
}

pub fn hide(app: &AppHandle) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    let expected = crate::surface::status(app);
    let hiding = app.clone();
    app.run_on_main_thread(move || {
        if expected.phase != "idle" || crate::surface::status(&hiding) != expected {
            return;
        }
        let generation = VISIBILITY.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = hiding.emit("overlay-hiding", ());
        tauri::async_runtime::spawn_blocking(move || {
            std::thread::sleep(std::time::Duration::from_millis(180));
            let app = hiding.clone();
            let _ = hiding.run_on_main_thread(move || {
                if VISIBILITY.load(Ordering::SeqCst) == generation
                    && crate::surface::status(&app).phase == "idle"
                    && let Some(window) = app.get_webview_window("main")
                    && let Err(error) = window.hide()
                {
                    log::warn!("[overlay] hide: {error}");
                }
            });
        });
    })
    .map_err(|error| error.to_string())
}

pub fn show(app: &AppHandle) -> Result<(), String> {
    let expected = crate::surface::status(app);
    let position = *app
        .state::<Mutex<OverlayPosition>>()
        .lock()
        .map_err(|e| e.to_string())?;
    let window = app
        .get_webview_window("main")
        .ok_or("Overlay unavailable")?;
    let monitor = focused_window_center()
        .and_then(|(x, y)| window.monitor_from_point(x, y).ok().flatten())
        .or(window.primary_monitor().map_err(|e| e.to_string())?)
        .ok_or("No screen available")?;
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let area = monitor.work_area();
    let margin = (24.0 * monitor.scale_factor()) as i32;
    let x = area.position.x + (area.size.width as i32 - size.width as i32) / 2;
    let y = match position {
        OverlayPosition::Top => area.position.y + margin,
        OverlayPosition::Bottom => {
            area.position.y + area.size.height as i32 - size.height as i32 - margin
        }
    };
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    {
        let native = window.clone();
        window
            .run_on_main_thread(move || {
                if !show_is_current(&expected, &crate::surface::status(native.app_handle())) {
                    return;
                }
                VISIBILITY.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let _ = native.emit("overlay-showing", ());
                if let Ok(pointer) = native.ns_window() {
                    // SAFETY: Tauri owns this NSWindow; the clone keeps it alive on the main thread.
                    let window = unsafe { &*pointer.cast::<objc2_app_kit::NSWindow>() };
                    window.orderFrontRegardless();
                }
            })
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let native = window.clone();
        window
            .run_on_main_thread(move || {
                if show_is_current(&expected, &crate::surface::status(native.app_handle())) {
                    VISIBILITY.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let _ = native.emit("overlay-showing", ());
                    let _ = native.show();
                }
            })
            .map_err(|e| e.to_string())
    }
}

pub fn resize(app: &AppHandle, width: f64, height: f64) -> Result<(), String> {
    if !width.is_finite()
        || !height.is_finite()
        || !(1.0..=640.0).contains(&width)
        || !(1.0..=240.0).contains(&height)
    {
        return Err("Invalid overlay size".into());
    }
    let window = app
        .get_webview_window("main")
        .ok_or("Overlay unavailable")?;
    let old = window.outer_size().map_err(|e| e.to_string())?;
    let origin = window.outer_position().map_err(|e| e.to_string())?;
    window
        .set_size(LogicalSize::new(width, height))
        .map_err(|e| e.to_string())?;
    let new = window.outer_size().map_err(|e| e.to_string())?;
    window
        .set_position(PhysicalPosition::new(
            origin.x + (old.width as i32 - new.width as i32) / 2,
            match *app
                .state::<Mutex<OverlayPosition>>()
                .lock()
                .map_err(|e| e.to_string())?
            {
                OverlayPosition::Top => origin.y,
                OverlayPosition::Bottom => origin.y + old.height as i32 - new.height as i32,
            },
        ))
        .map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
fn focused_window_center() -> Option<(f64, f64)> {
    None
}

/// How long one Accessibility call may wait on the focused app. The default is
/// about 6 s, which would stall the hotkey on a hung app; past this we fall
/// back to the main display.
#[cfg(target_os = "macos")]
const AX_TIMEOUT_SECONDS: f32 = 0.1;

#[cfg(target_os = "macos")]
fn focused_window_center() -> Option<(f64, f64)> {
    use core_foundation::{
        base::{CFType, CFTypeID, CFTypeRef, TCFType},
        string::{CFString, CFStringRef},
    };
    use core_graphics::geometry::{CGPoint, CGSize};
    use std::ffi::c_void;
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXUIElementCreateSystemWide() -> CFTypeRef;
        fn AXUIElementSetMessagingTimeout(element: CFTypeRef, timeout: f32) -> i32;
        fn AXUIElementCopyAttributeValue(
            element: CFTypeRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> i32;
        fn AXValueGetTypeID() -> CFTypeID;
        fn AXValueGetType(value: CFTypeRef) -> u32;
        fn AXValueGetValue(value: CFTypeRef, kind: u32, output: *mut c_void) -> bool;
    }
    fn attribute(element: &CFType, name: &str) -> Option<CFType> {
        let mut value = std::ptr::null();
        // SAFETY: element and key remain alive for the call; successful Copy returns an owned CF object.
        let result = unsafe {
            AXUIElementCopyAttributeValue(
                element.as_CFTypeRef(),
                CFString::new(name).as_concrete_TypeRef(),
                &mut value,
            )
        };
        if result != 0 || value.is_null() {
            return None;
        }
        // SAFETY: checked successful Copy ownership above.
        Some(unsafe { CFType::wrap_under_create_rule(value) })
    }
    // SAFETY: CreateSystemWide takes no arguments and returns an owned accessibility element.
    let system = unsafe { CFType::wrap_under_create_rule(AXUIElementCreateSystemWide()) };
    let limit = |element: &CFType| {
        // SAFETY: element is a live AXUIElement for the duration of the call.
        unsafe { AXUIElementSetMessagingTimeout(element.as_CFTypeRef(), AX_TIMEOUT_SECONDS) }
    };
    limit(&system);
    let app = attribute(&system, "AXFocusedApplication")?;
    limit(&app);
    let window = attribute(&app, "AXFocusedWindow")?;
    let position = attribute(&window, "AXPosition")?;
    let size = attribute(&window, "AXSize")?;
    let mut point = CGPoint::new(0.0, 0.0);
    let mut dimensions = CGSize::new(0.0, 0.0);
    // SAFETY: AXValue type checks establish the output layouts: CGPoint=1 and CGSize=2.
    let valid = unsafe {
        position.type_of() == AXValueGetTypeID()
            && size.type_of() == AXValueGetTypeID()
            && AXValueGetType(position.as_CFTypeRef()) == 1
            && AXValueGetType(size.as_CFTypeRef()) == 2
            && AXValueGetValue(
                position.as_CFTypeRef(),
                1,
                (&mut point as *mut CGPoint).cast(),
            )
            && AXValueGetValue(
                size.as_CFTypeRef(),
                2,
                (&mut dimensions as *mut CGSize).cast(),
            )
    };
    valid.then_some((
        point.x + dimensions.width / 2.0,
        point.y + dimensions.height / 2.0,
    ))
}

#[cfg(test)]
mod tests {
    use super::show_is_current;
    use crate::{problem::Problem, surface::Status};

    #[test]
    fn error_show_survives_worker_completion_but_not_dismissal_or_replacement() {
        let expected = Status {
            phase: "transcribing".into(),
            error: Some(Problem::new("Couldn’t paste", "Copy from History.")),
            notice: None,
        };
        let mut current = expected.clone();
        current.phase = "idle".into();
        assert!(show_is_current(&expected, &current));
        current.error = None;
        assert!(!show_is_current(&expected, &current));
        current.error = Some(Problem::new("Another problem", "Open Settings."));
        assert!(!show_is_current(&expected, &current));
    }

    #[test]
    fn a_recording_show_cannot_reopen_a_completed_or_discarded_take() {
        let expected = Status {
            phase: "recording".into(),
            ..Status::default()
        };
        assert!(show_is_current(&expected, &expected));
        for phase in ["idle", "transcribing", "done", "copied"] {
            let current = Status {
                phase: phase.into(),
                ..Status::default()
            };
            assert!(!show_is_current(&expected, &current));
        }
    }
}
