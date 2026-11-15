use crate::settings::Settings;
use tauri::{
    App, AppHandle, Emitter, Manager,
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

use serde::Serialize;
use std::sync::Mutex;

#[derive(Clone, Default, Serialize)]
pub struct Status {
    pub phase: String,
    pub error: Option<String>,
}

pub fn status(app: &AppHandle) -> Status {
    app.state::<Mutex<Status>>()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

pub fn set_error(app: &AppHandle, error: &str) {
    app.state::<Mutex<Status>>()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .error = Some(error.into());
    refresh_tray(app);
}

pub fn clear_error(app: &AppHandle) {
    app.state::<Mutex<Status>>()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .error = None;
    refresh_tray(app);
    if let Err(error) = app.emit("dictation-state", status(app).phase) {
        eprintln!("[tray] {error}");
    }
}

struct ShortcutItem(MenuItem<tauri::Wry>);

fn icon(phase: &str) -> Image<'static> {
    let mut pixels = vec![0; 22 * 22 * 4];
    let heights = match phase {
        "error" => [20, 4, 20, 4, 20],
        "recording" => [6, 14, 20, 12, 6],
        "transcribing" => [12, 6, 12, 6, 12],
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
            eprintln!("[tray] {error}");
        }
        let tooltip = status.error.map_or_else(
            || format!("dictate · {phase}"),
            |error| format!("dictate · {error}"),
        );
        if let Err(error) = tray.set_tooltip(Some(tooltip)) {
            eprintln!("[tray] {error}");
        }
    }
}

pub fn set_phase(app: &AppHandle, phase: &str) {
    app.state::<Mutex<Status>>()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .phase = phase.into();
    refresh_tray(app);
    if let Err(error) = app.emit("dictation-state", phase) {
        eprintln!("[tray] {error}");
    }
}

fn shortcut_label(shortcut: &str) -> String {
    shortcut
        .split('+')
        .map(|key| match key {
            "Alt" | "Option" => "⌥",
            "Shift" => "⇧",
            "Control" => "⌃",
            "Super" | "Meta" | "CommandOrControl" => "⌘",
            key => key
                .strip_prefix("Key")
                .or_else(|| key.strip_prefix("Digit"))
                .unwrap_or(key),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn update_shortcut(app: &AppHandle, shortcut: &str) {
    if let Err(error) = app
        .state::<ShortcutItem>()
        .0
        .set_text(format!("Shortcut: {}", shortcut_label(shortcut)))
    {
        eprintln!("[tray] {error}");
    }
}

pub fn setup(app: &App, settings: &Settings) -> tauri::Result<()> {
    app.manage(Mutex::new(Status {
        phase: "idle".into(),
        error: None,
    }));
    let open = MenuItem::with_id(app, "settings", "Open Settings…", true, None::<&str>)?;
    let shortcut = MenuItem::with_id(
        app,
        "shortcut",
        format!("Shortcut: {}", shortcut_label(&settings.shortcut)),
        false,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit dictate", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &shortcut, &separator, &quit])?;
    app.manage(ShortcutItem(shortcut));
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
                    eprintln!("[settings] {error}");
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_menu_uses_mac_symbols_and_readable_key_names() {
        assert_eq!(shortcut_label("Alt+Shift+Space"), "⌥ ⇧ Space");
        assert_eq!(shortcut_label("Super+Control+KeyD"), "⌘ ⌃ D");
    }
}
