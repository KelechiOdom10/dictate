use crate::settings::Settings;
use tauri::{
    App, AppHandle, Emitter, Manager,
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

struct ShortcutItem(MenuItem<tauri::Wry>);

fn icon(phase: &str) -> Image<'static> {
    let mut pixels = vec![0; 22 * 22 * 4];
    let heights = match phase {
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

pub fn set_phase(app: &AppHandle, phase: &str) {
    if let Some(tray) = app.tray_by_id("dictate") {
        if let Err(error) = tray.set_icon_with_as_template(Some(icon(phase)), true) {
            eprintln!("[tray] {error}");
        }
        if let Err(error) = tray.set_tooltip(Some(format!("dictate · {phase}"))) {
            eprintln!("[tray] {error}");
        }
    }
    if let Err(error) = app.emit("dictation-state", phase) {
        eprintln!("[tray] {error}");
    }
}

pub fn update_shortcut(app: &AppHandle, shortcut: &str) {
    if let Err(error) = app
        .state::<ShortcutItem>()
        .0
        .set_text(format!("Shortcut: {shortcut}"))
    {
        eprintln!("[tray] {error}");
    }
}

pub fn setup(app: &App, settings: &Settings) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "settings", "Open Settings…", true, None::<&str>)?;
    let shortcut = MenuItem::with_id(
        app,
        "shortcut",
        format!("Shortcut: {}", settings.shortcut),
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
