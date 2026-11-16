use crate::problem::Problem;
use crate::settings::Settings;
use tauri::{
    App, AppHandle, Emitter, Manager,
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
};

use serde::Serialize;
use std::sync::Mutex;

#[derive(Clone, Default, PartialEq, Serialize)]
pub struct Status {
    pub phase: String,
    pub error: Option<Problem>,
    pub notice: Option<String>,
}

impl Status {
    pub fn should_hide(&self) -> bool {
        self.phase == "idle" && self.error.is_none()
    }
}

pub fn status(app: &AppHandle) -> Status {
    app.state::<Mutex<Status>>()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

pub fn set_notice(app: &AppHandle, notice: Option<String>) {
    app.state::<Mutex<Status>>()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .notice = notice;
}

pub fn set_error(app: &AppHandle, error: &Problem) {
    app.state::<Mutex<Status>>()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .error = Some(error.clone());
    refresh_tray(app);
}

pub fn clear_error(app: &AppHandle) {
    app.state::<Mutex<Status>>()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .error = None;
    refresh_tray(app);
    if let Err(error) = app.emit("dictation-state", status(app).phase) {
        log::warn!("[tray] {error}");
    }
}

fn icon(phase: &str) -> Image<'static> {
    let mut pixels = vec![0; 22 * 22 * 4];
    let heights = match phase {
        "error" => [20, 4, 20, 4, 20],
        "recording" | "annotating" => [6, 14, 20, 12, 6],
        "transcribing" | "packaging" => [12, 6, 12, 6, 12],
        _ => [4, 8, 12, 8, 4],
    };
    for (bar, height) in heights.into_iter().enumerate() {
        for y in (22 - height) / 2..(22 + height) / 2 {
            for x in 2 + bar * 4..4 + bar * 4 {
                pixels[(y * 22 + x) * 4 + 3] = 255;
            }
        }
    }
    Image::new_owned(pixels, 22, 22)
}

fn tray_presentation(status: &Status) -> (&str, String) {
    let phase = if status.error.is_some() {
        "error"
    } else {
        &status.phase
    };
    let tooltip = status.error.as_ref().map_or_else(
        || format!("dictate · {phase}"),
        |error| format!("dictate · {}", error.title),
    );
    (phase, tooltip)
}

fn dispatch_tray_refresh(
    dispatch: impl FnOnce(Box<dyn FnOnce() + Send>) -> Result<(), String>,
    read: impl FnOnce() -> Status + Send + 'static,
    apply: impl FnOnce(Status) + Send + 'static,
) -> Result<(), String> {
    dispatch(Box::new(move || apply(read())))
}

fn refresh_tray(app: &AppHandle) {
    let reading = app.clone();
    let applying = app.clone();
    if let Err(error) = dispatch_tray_refresh(
        |task| {
            app.run_on_main_thread(task)
                .map_err(|error| error.to_string())
        },
        move || status(&reading),
        move |status| {
            let (phase, tooltip) = tray_presentation(&status);
            if let Some(tray) = applying.tray_by_id("dictate") {
                if let Err(error) = tray.set_icon_with_as_template(Some(icon(phase)), true) {
                    log::warn!("[tray] icon: {error}");
                }
                if let Err(error) = tray.set_tooltip(Some(tooltip)) {
                    log::warn!("[tray] tooltip: {error}");
                }
                log::info!(
                    "[tray] refresh phase={} error={}",
                    status.phase,
                    status.error.is_some()
                );
            } else {
                log::warn!("[tray] icon unavailable");
            }
        },
    ) {
        log::warn!("[tray] dispatch: {error}");
    }
}

/// Tell the overlay to read the status again, after a notice changed.
pub fn refresh(app: &AppHandle) {
    if let Err(error) = app.emit("dictation-state", status(app).phase) {
        log::warn!("[tray] {error}");
    }
}

pub fn set_phase(app: &AppHandle, phase: &str) {
    app.state::<Mutex<Status>>()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .phase = phase.into();
    refresh_tray(app);
    if let Err(error) = app.emit("dictation-state", phase) {
        log::warn!("[tray] {error}");
    }
}

pub fn update_shortcut(app: &AppHandle, _shortcut: &str) {
    refresh_menu(app);
}

fn menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let version = MenuItem::with_id(
        app,
        "version",
        format!("dictate {}", app.package_info().version),
        false,
        None::<&str>,
    )?;
    let paste = MenuItem::with_id(
        app,
        "paste-last",
        "Paste Last Dictation",
        true,
        None::<&str>,
    )?;
    let copy = MenuItem::with_id(app, "copy-last", "Copy Last Transcript", true, None::<&str>)?;
    let models = Submenu::with_id(app, "speech-model", "Speech Model", true)?;
    let active = crate::get_settings(app.clone())
        .map(|s| s.speech_model)
        .ok();
    let dir = &app.state::<crate::Models>().dir;
    let mut installed = 0;
    for (index, info) in crate::models::CATALOG.iter().enumerate() {
        if crate::models::path(dir, info.id).is_file() {
            models.append(&CheckMenuItem::with_id(
                app,
                format!("model-{index}"),
                crate::models::label(info),
                true,
                active == Some(info.id),
                None::<&str>,
            )?)?;
            installed += 1;
        }
    }
    if installed == 0 {
        models.append(&MenuItem::with_id(
            app,
            "no-models",
            "No installed models",
            false,
            None::<&str>,
        )?)?;
    }
    let open = MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?;
    let updates = MenuItem::with_id(app, "updates", "Check for Updates…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit dictate", true, Some("CmdOrCtrl+Q"))?;
    Menu::with_items(
        app,
        &[
            &version,
            &paste,
            &copy,
            &models,
            &PredefinedMenuItem::separator(app)?,
            &open,
            &updates,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )
}

pub fn refresh_menu(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id("dictate") {
        match menu(app).and_then(|menu| tray.set_menu(Some(menu))) {
            Ok(()) => {}
            Err(error) => log::warn!("[tray] {error}"),
        }
    }
}

#[cfg(target_os = "macos")]
fn alert(title: &str, detail: &str) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSAlert, NSApplication};
    use objc2_foundation::NSString;

    let Some(main_thread) = MainThreadMarker::new() else {
        log::warn!("[tray] alert requested off the main thread");
        return;
    };
    NSApplication::sharedApplication(main_thread).activate();
    let alert = NSAlert::new(main_thread);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(detail));
    alert.runModal();
}

#[cfg(not(target_os = "macos"))]
fn alert(title: &str, detail: &str) {
    log::info!("[tray] {title}: {detail}");
}

fn handle_menu(app: &AppHandle, id: &str) -> Result<(), String> {
    match id {
        "paste-last" | "copy-last" => {
            let entries = crate::list_history(app.clone(), String::new(), 100)?.entries;
            let text = entries
                .into_iter()
                .find(|entry| !entry.text.trim().is_empty())
                .ok_or("No transcript in History. Dictate with History enabled first.")?
                .text;
            if id == "copy-last" {
                crate::clipboard::copy_text(&text)
            } else {
                let settings = crate::get_settings(app.clone())?;
                crate::insert::paste(
                    app,
                    &text,
                    crate::insert::Delivery::from_settings(&settings),
                )
                .map(|_| ())
                .map_err(|e| e.to_string())
            }
        }
        "updates" => app
            .run_on_main_thread(|| {
                alert(
                    "Updates aren't set up yet",
                    "This build of dictate can't check for updates. Install new versions by hand for now.",
                )
            })
            .map_err(|e| e.to_string()),
        id if id.starts_with("model-") => {
            let index: usize = id
                .trim_start_matches("model-")
                .parse::<usize>()
                .map_err(|e| e.to_string())?;
            let model = crate::models::CATALOG.get(index).ok_or("Unknown model")?;
            let mut settings = crate::get_settings(app.clone())?;
            settings.speech_model = model.id;
            crate::save_settings(app.clone(), settings)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

pub fn setup(app: &App, _settings: &Settings) -> tauri::Result<()> {
    app.manage(Mutex::new(Status {
        phase: "idle".into(),
        error: None,
        notice: None,
    }));
    let menu = menu(app.handle())?;
    TrayIconBuilder::with_id("dictate")
        .icon(icon("idle"))
        .icon_as_template(true)
        .tooltip("dictate · idle")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => {
                crate::show_settings(app);
            }
            "quit" => app.exit(0),
            id => {
                if let Err(error) = handle_menu(app, id) {
                    crate::report_error(app, &Problem::new("Menu action failed", error));
                }
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    type Queue = Vec<Box<dyn FnOnce() + Send>>;

    #[test]
    fn refresh_returns_before_reading_or_rendering_while_the_runtime_is_locked() {
        let runtime = Arc::new(Mutex::new(()));
        let held = runtime.lock().unwrap();
        let reading = runtime.clone();
        let applied = Arc::new(Mutex::new(false));
        let rendered = applied.clone();
        let mut queue = Queue::new();
        dispatch_tray_refresh(
            |task| {
                queue.push(task);
                Ok(())
            },
            move || {
                let _runtime = reading
                    .try_lock()
                    .expect("read ran before main-thread dispatch");
                Status {
                    phase: "idle".into(),
                    ..Status::default()
                }
            },
            move |_| {
                *rendered.lock().unwrap() = true;
            },
        )
        .unwrap();
        assert!(!*applied.lock().unwrap());
        drop(held);
        queue.pop().unwrap()();
        assert!(*applied.lock().unwrap());
    }

    #[test]
    fn reversed_refreshes_read_the_current_completion_instead_of_stale_activity() {
        for terminal in ["done", "copied", "skipped", "idle"] {
            let state = Arc::new(Mutex::new(Status::default()));
            let rendered = Arc::new(Mutex::new(Vec::new()));
            let mut queue = Queue::new();
            for phase in ["recording", "transcribing", terminal] {
                state.lock().unwrap().phase = phase.into();
                let reading = state.clone();
                let applying = rendered.clone();
                dispatch_tray_refresh(
                    |task| {
                        queue.push(task);
                        Ok(())
                    },
                    move || reading.lock().unwrap().clone(),
                    move |status| {
                        let (phase, tooltip) = tray_presentation(&status);
                        applying
                            .lock()
                            .unwrap()
                            .push((icon(phase).rgba().to_vec(), tooltip));
                    },
                )
                .unwrap();
            }
            while let Some(task) = queue.pop() {
                task();
            }
            let expected = (
                icon("idle").rgba().to_vec(),
                format!("dictate · {terminal}"),
            );
            assert_eq!(*rendered.lock().unwrap(), vec![expected; 3]);
        }
    }

    #[test]
    fn delayed_error_and_clear_refreshes_keep_icon_and_tooltip_from_the_same_status() {
        for cleared in [false, true] {
            let state = Arc::new(Mutex::new(Status {
                phase: "idle".into(),
                ..Status::default()
            }));
            let rendered = Arc::new(Mutex::new(Vec::new()));
            let mut queue = Queue::new();
            let mut errors = vec![
                None,
                Some(Problem::new("Permission needed", "Open Settings.")),
            ];
            if cleared {
                errors.push(None);
            }
            let count = errors.len();
            for error in errors {
                state.lock().unwrap().error = error;
                let reading = state.clone();
                let applying = rendered.clone();
                dispatch_tray_refresh(
                    |task| {
                        queue.push(task);
                        Ok(())
                    },
                    move || reading.lock().unwrap().clone(),
                    move |status| {
                        let (phase, tooltip) = tray_presentation(&status);
                        applying
                            .lock()
                            .unwrap()
                            .push((icon(phase).rgba().to_vec(), tooltip));
                    },
                )
                .unwrap();
            }
            while let Some(task) = queue.pop() {
                task();
            }
            let expected = if cleared {
                (icon("idle").rgba().to_vec(), "dictate · idle".to_string())
            } else {
                (
                    icon("error").rgba().to_vec(),
                    "dictate · Permission needed".to_string(),
                )
            };
            assert_eq!(*rendered.lock().unwrap(), vec![expected; count]);
        }
    }

    #[test]
    fn common_tray_artwork_covers_every_activity_and_error_path() {
        for (phase, artwork) in [
            ("idle", "idle"),
            ("done", "idle"),
            ("copied", "idle"),
            ("skipped", "idle"),
            ("recording", "recording"),
            ("annotating", "recording"),
            ("transcribing", "transcribing"),
            ("packaging", "transcribing"),
        ] {
            let mut status = Status {
                phase: phase.into(),
                ..Status::default()
            };
            let (selected, tooltip) = tray_presentation(&status);
            assert_eq!(icon(selected).rgba(), icon(artwork).rgba());
            assert_eq!(tooltip, format!("dictate · {phase}"));
            status.error = Some(Problem::new("Permission needed", "Open Settings."));
            let (selected, tooltip) = tray_presentation(&status);
            assert_eq!(icon(selected).rgba(), icon("error").rgba());
            assert_eq!(tooltip, "dictate · Permission needed");
        }
    }
}
