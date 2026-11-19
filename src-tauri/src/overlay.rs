use crate::focus::{focused_window_center, pointer};
use crate::settings::OverlayPosition;
use std::sync::Mutex;
use tauri::{App, AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition};
use tauri_nspanel::WebviewWindowExt;

mod panel {
    tauri_nspanel::tauri_panel! {
        panel!(CapsulePanel {
            config: {
                can_become_key_window: false,
                can_become_main_window: false,
                is_floating_panel: true
            }
        })
    }
}

/// Every Space, beside full-screen apps, and fixed during Space switches.
pub fn collection_behavior() -> objc2_app_kit::NSWindowCollectionBehavior {
    use objc2_app_kit::NSWindowCollectionBehavior as Behavior;
    Behavior::CanJoinAllSpaces | Behavior::FullScreenAuxiliary | Behavior::Stationary
}

/// Above pop-up menus, Split View and full-screen apps. The capsule and the
/// Annotate bar share it; the ink panel sits one below, under both.
pub const LEVEL: i64 = 1001;

/// Puts `window` back on every Space that exists now, full-screen ones
/// included. Call it on the main thread just before ordering an overlay front.
///
/// `collection_behavior` is set once at launch, but the window server keeps
/// its own list of the Spaces a long-lived panel belongs to, and that list can
/// go stale: on 18 Nov 2026 the panels belonged to the desktop and one
/// full-screen Space but not another, and `orderFrontRegardless` on that Space
/// left them off screen. Setting the same behaviour again changes nothing,
/// because AppKit sends it to the window server only when the transaction
/// commits and two writes in one transaction cancel out. Committing a
/// different behaviour first makes the second write rebuild the list.
#[cfg(target_os = "macos")]
pub fn join_all_spaces(window: &objc2_app_kit::NSWindow) {
    window.setCollectionBehavior(objc2_app_kit::NSWindowCollectionBehavior::Default);
    objc2_quartz_core::CATransaction::flush();
    window.setCollectionBehavior(collection_behavior());
}

/// Turns the capsule into a non-activating panel, so it can show over a
/// full-screen app, an open menu or on any Space without taking focus from the
/// app the user dictates into. Called once from `setup`, on the main thread.
pub fn setup(app: &App) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("main") else {
        return Err(tauri::Error::WindowNotFound);
    };
    window.set_ignore_cursor_events(false)?;
    window.set_focusable(false)?;
    let panel = window.to_panel::<panel::CapsulePanel>()?;
    panel
        .add_style_mask(objc2_app_kit::NSWindowStyleMask::NonactivatingPanel)
        .map_err(|e| tauri::Error::Io(std::io::Error::other(format!("{e:?}"))))?;
    panel.set_level(LEVEL);
    panel.set_collection_behavior(collection_behavior());
    panel.set_hides_on_deactivate(false);
    panel.set_transparent(true);
    panel.set_has_shadow(false);
    Ok(())
}

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
    let monitor = [focused_window_center(), pointer()]
        .into_iter()
        .flatten()
        .find_map(|(x, y)| window.monitor_from_point(x, y).ok().flatten())
        .or(window.primary_monitor().map_err(|e| e.to_string())?)
        .ok_or("No screen available")?;
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let area = monitor.work_area();
    let clearance = if expected.phase == "annotating" {
        crate::ink::BAR_CLEARANCE
    } else {
        0.0
    };
    let margin = ((24.0 + clearance) * monitor.scale_factor()) as i32;
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
                    join_all_spaces(window);
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
