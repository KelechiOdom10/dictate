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

/// The artwork a phase shows. Phases that share artwork share a key, so the
/// tray only installs a new image when the key changes.
fn artwork(phase: &str) -> &'static str {
    match phase {
        "error" => "error",
        "recording" | "annotating" => "recording",
        "transcribing" | "packaging" => "transcribing",
        _ => "idle",
    }
}

/// Animation frames per loop, shown at `FRAME_INTERVAL`: about 8 fps.
const FRAMES: usize = 8;
const FRAME_INTERVAL: std::time::Duration = std::time::Duration::from_millis(125);

/// Bar heights for an artwork. `None` is the static artwork; a frame index is
/// one step of the recording pulse or the transcribing sweep.
fn heights(artwork: &str, frame: Option<usize>) -> [usize; 5] {
    let wave = |bar: usize, frame: usize| {
        ((frame as f64 / FRAMES as f64 - bar as f64 / 5.0) * std::f64::consts::TAU).cos()
    };
    match (artwork, frame) {
        ("error", _) => [20, 4, 20, 4, 20],
        ("recording", None) => [6, 14, 20, 12, 6],
        ("recording", Some(frame)) => std::array::from_fn(|bar| {
            let base = [6.0, 14.0, 20.0, 12.0, 6.0][bar];
            (base * (0.8 + 0.2 * wave(bar, frame))).round().max(4.0) as usize
        }),
        ("transcribing", None) => [12, 6, 12, 6, 12],
        ("transcribing", Some(frame)) => {
            std::array::from_fn(|bar| 6 + (8.0 * wave(bar, frame).max(0.0)).round() as usize)
        }
        _ => [4, 8, 12, 8, 4],
    }
}

fn frame_icon(artwork: &str, frame: Option<usize>) -> Image<'static> {
    let mut pixels = vec![0; 22 * 22 * 4];
    for (bar, height) in heights(artwork, frame).into_iter().enumerate() {
        for y in (22 - height) / 2..(22 + height) / 2 {
            for x in 2 + bar * 4..4 + bar * 4 {
                pixels[(y * 22 + x) * 4 + 3] = 255;
            }
        }
    }
    Image::new_owned(pixels, 22, 22)
}

fn icon(phase: &str) -> Image<'static> {
    frame_icon(artwork(phase), None)
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

/// What the tray shows now. `frame` is `Some` while an animation runs, and
/// `generation` changes with every artwork change so a running timer can tell
/// it is stale.
#[derive(Clone, Debug, PartialEq)]
struct Shown {
    artwork: &'static str,
    frame: Option<usize>,
    tooltip: String,
    generation: u64,
}

/// The tray's current presentation, shared by refreshes and the frame timer.
struct TrayShown(Mutex<Shown>);

/// What a refresh must do: install an image, set a tooltip, start a timer.
#[derive(Debug, Default, PartialEq)]
struct Changes {
    icon: Option<(&'static str, Option<usize>)>,
    tooltip: Option<String>,
    timer: Option<u64>,
}

/// A new image only when the artwork changes, a new tooltip only when the text
/// changes, and one timer per animated artwork, never one per refresh.
fn tray_changes(shown: &mut Shown, status: &Status, reduce_motion: bool) -> Changes {
    let (phase, tooltip) = tray_presentation(status);
    let artwork = artwork(phase);
    let mut changes = Changes::default();
    if shown.artwork != artwork {
        let animated = !reduce_motion && matches!(artwork, "recording" | "transcribing");
        shown.artwork = artwork;
        shown.frame = animated.then_some(0);
        shown.generation += 1;
        changes.icon = Some((artwork, shown.frame));
        changes.timer = animated.then_some(shown.generation);
    }
    if shown.tooltip != tooltip {
        shown.tooltip = tooltip.clone();
        changes.tooltip = Some(tooltip);
    }
    changes
}

/// The next frame for the timer started at `generation`, or `None` once the
/// artwork has changed and that timer must stop.
fn advance(shown: &mut Shown, generation: u64) -> Option<(&'static str, usize)> {
    if shown.generation != generation {
        return None;
    }
    let frame = (shown.frame? + 1) % FRAMES;
    shown.frame = Some(frame);
    Some((shown.artwork, frame))
}

/// The frame timer: wait, take the next frame, install it, until there is none.
fn animate(
    mut wait: impl FnMut(),
    mut next: impl FnMut() -> Option<(&'static str, usize)>,
    mut install: impl FnMut(&'static str, usize),
) {
    loop {
        wait();
        let Some((artwork, frame)) = next() else {
            return;
        };
        install(artwork, frame);
    }
}

#[cfg(target_os = "macos")]
fn reduce_motion() -> bool {
    objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

#[cfg(not(target_os = "macos"))]
fn reduce_motion() -> bool {
    false
}

fn shown(app: &AppHandle) -> std::sync::MutexGuard<'_, Shown> {
    app.state::<TrayShown>()
        .inner()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn start_animation(app: AppHandle, generation: u64) {
    std::thread::spawn(move || {
        animate(
            || std::thread::sleep(FRAME_INTERVAL),
            || advance(&mut shown(&app), generation),
            |artwork, frame| {
                let installing = app.clone();
                let dispatched = app.run_on_main_thread(move || {
                    if shown(&installing).generation != generation {
                        return;
                    }
                    if let Some(tray) = installing.tray_by_id("dictate")
                        && let Err(error) = tray
                            .set_icon_with_as_template(Some(frame_icon(artwork, Some(frame))), true)
                    {
                        log::warn!("[tray] frame: {error}");
                    }
                });
                if let Err(error) = dispatched {
                    log::warn!("[tray] frame dispatch: {error}");
                }
            },
        );
    });
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
            if let Some(tray) = applying.tray_by_id("dictate") {
                let changes = tray_changes(&mut shown(&applying), &status, reduce_motion());
                if let Some((artwork, frame)) = changes.icon
                    && let Err(error) =
                        tray.set_icon_with_as_template(Some(frame_icon(artwork, frame)), true)
                {
                    log::warn!("[tray] icon: {error}");
                }
                if let Some(generation) = changes.timer {
                    start_animation(applying.clone(), generation);
                }
                if let Some(tooltip) = changes.tooltip
                    && let Err(error) = tray.set_tooltip(Some(tooltip))
                {
                    log::warn!("[tray] tooltip: {error}");
                }
                log::info!(
                    "[tray] refresh phase={} error={} icon={}",
                    status.phase,
                    status.error.is_some(),
                    changes.icon.map_or("unchanged", |(artwork, _)| artwork)
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
    app.manage(TrayShown(Mutex::new(Shown {
        artwork: "idle",
        frame: None,
        tooltip: "dictate · idle".into(),
        generation: 0,
    })));
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

    fn idle() -> Shown {
        Shown {
            artwork: "idle",
            frame: None,
            tooltip: "dictate · idle".into(),
            generation: 0,
        }
    }

    fn at(phase: &str) -> Status {
        Status {
            phase: phase.into(),
            ..Status::default()
        }
    }

    #[test]
    fn a_phase_transition_never_installs_the_same_icon_twice() {
        let mut shown = idle();
        let mut installed = Vec::new();
        let mut tooltips = 0;
        let mut status = Status::default();
        let steps: [(&str, Option<Problem>); 11] = [
            ("idle", None),
            ("recording", None),
            ("recording", None),
            ("transcribing", None),
            ("copied", None),
            ("idle", None),
            ("annotating", None),
            ("packaging", None),
            (
                "idle",
                Some(Problem::new("Permission needed", "Open Settings.")),
            ),
            (
                "recording",
                Some(Problem::new("Permission needed", "Open Settings.")),
            ),
            ("idle", None),
        ];
        for (phase, error) in steps {
            status.phase = phase.into();
            status.error = error;
            let changes = tray_changes(&mut shown, &status, false);
            installed.extend(changes.icon);
            tooltips += usize::from(changes.tooltip.is_some());
        }
        assert_eq!(
            installed,
            [
                ("recording", Some(0)),
                ("transcribing", Some(0)),
                ("idle", None),
                ("recording", Some(0)),
                ("transcribing", Some(0)),
                ("error", None),
                ("idle", None),
            ]
        );
        assert!(installed.windows(2).all(|pair| pair[0] != pair[1]));
        assert_eq!(tooltips, 8);
        assert_eq!((shown.artwork, shown.frame), ("idle", None));
    }

    #[test]
    fn frames_advance_on_the_timer_while_recording_and_the_timer_stops_at_idle() {
        let shown = Mutex::new(idle());
        let generation = tray_changes(&mut shown.lock().unwrap(), &at("recording"), false)
            .timer
            .expect("recording starts one timer");
        assert_eq!(
            tray_changes(&mut shown.lock().unwrap(), &at("recording"), false),
            Changes::default()
        );
        let mut ticks = 0;
        let mut installed = Vec::new();
        animate(
            || {
                ticks += 1;
                if ticks == FRAMES + 3 {
                    let changes = tray_changes(&mut shown.lock().unwrap(), &at("copied"), false);
                    assert_eq!(changes.icon, Some(("idle", None)));
                    assert_eq!(changes.timer, None);
                }
            },
            || advance(&mut shown.lock().unwrap(), generation),
            |artwork, frame| installed.push((artwork, frame)),
        );
        let expected: Vec<_> = (1..FRAMES + 3)
            .map(|tick| ("recording", tick % FRAMES))
            .collect();
        assert_eq!(installed, expected);
        assert_eq!(ticks, FRAMES + 3);
        assert_eq!(advance(&mut shown.lock().unwrap(), generation), None);
        assert_eq!(shown.lock().unwrap().frame, None);
    }

    #[test]
    fn a_new_animated_phase_retires_the_previous_timer() {
        let mut shown = idle();
        let recording = tray_changes(&mut shown, &at("recording"), false)
            .timer
            .unwrap();
        let transcribing = tray_changes(&mut shown, &at("transcribing"), false)
            .timer
            .unwrap();
        assert_ne!(recording, transcribing);
        assert_eq!(advance(&mut shown, recording), None);
        assert_eq!(advance(&mut shown, transcribing), Some(("transcribing", 1)));
    }

    #[test]
    fn reduce_motion_keeps_the_static_artwork_and_starts_no_timer() {
        let mut shown = idle();
        for phase in ["recording", "transcribing"] {
            let changes = tray_changes(&mut shown, &at(phase), true);
            assert_eq!(changes.icon, Some((phase, None)));
            assert_eq!(changes.timer, None);
            let generation = shown.generation;
            assert_eq!(advance(&mut shown, generation), None);
            assert_eq!(frame_icon(phase, None).rgba(), icon(phase).rgba());
        }
    }

    #[test]
    fn animation_frames_fit_the_icon_and_move_every_step() {
        for artwork in ["recording", "transcribing"] {
            let frames: Vec<_> = (0..FRAMES)
                .map(|frame| heights(artwork, Some(frame)))
                .collect();
            assert!(
                frames
                    .iter()
                    .flatten()
                    .all(|height| (4..=20).contains(height))
            );
            for frame in 0..FRAMES {
                assert_ne!(
                    frames[frame],
                    frames[(frame + 1) % FRAMES],
                    "{artwork} {frame}"
                );
            }
        }
        assert_ne!(
            heights("recording", Some(0)),
            heights("transcribing", Some(0))
        );
    }
}
