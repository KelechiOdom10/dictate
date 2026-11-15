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

#[derive(Clone, Default, Serialize)]
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

fn refresh_tray(app: &AppHandle) {
    let status = status(app);
    let phase = if status.error.is_some() {
        "error"
    } else {
        &status.phase
    };
    if let Some(tray) = app.tray_by_id("dictate") {
        if let Err(error) = tray.set_icon_with_as_template(Some(icon(phase)), true) {
            log::warn!("[tray] {error}");
        }
        let tooltip = status.error.map_or_else(
            || format!("dictate · {phase}"),
            |error| format!("dictate · {}", error.title),
        );
        if let Err(error) = tray.set_tooltip(Some(tooltip)) {
            log::warn!("[tray] {error}");
        }
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
                info.name,
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
                .map_err(|e| e.to_string())
            }
        }
        "updates" => {
            set_notice(
                app,
                Some("Update checking is coming in a later release.".into()),
            );
            refresh(app);
            if let Some(window) = app.get_webview_window("settings") {
                window
                    .show()
                    .and_then(|()| window.set_focus())
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        }
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
                if let Some(window) = app.get_webview_window("settings")
                    && let Err(error) = window.show().and_then(|()| window.set_focus())
                {
                    log::warn!("[settings] {error}");
                }
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
