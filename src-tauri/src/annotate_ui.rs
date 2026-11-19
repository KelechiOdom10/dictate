//! The two windows Annotate shows while it records, and the threads behind
//! them. `ink.rs` holds the rules; this module reads the pointer and the
//! session clock, moves the windows, and tells the webviews what to draw.
//!
//! - `ink`: a transparent, click-through panel over the recorded display. It
//!   draws the ink and, in pen mode only, takes the mouse.
//! - `annotate-bar`: the floating bar with the timer, pen and stop buttons.
//!   After a session it comes back for a few seconds as "Copied", with
//!   "Copy image" for the contact sheet.
//!
//! Both are non-activating `NSPanel`s (`tauri-nspanel`, as in spike #13) that
//! can never become key, so clicking or drawing on them never activates
//! dictate and never takes focus from the app being annotated.
//!
//! Nothing here sends input to other apps. The pointer is read with
//! `CGEvent::location` on a fresh event object that is never posted.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{App, AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_nspanel::{ManagerExt, WebviewWindowExt};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut};

use crate::focus::{pointer, shift_and_button};
use crate::ink::{self, Ink, Pen, PenInput, Release, ShiftButton};
use crate::package::{Display, Stroke};
use crate::settings::OverlayPosition;

pub const INK: &str = "ink";
pub const BAR: &str = "annotate-bar";

/// The backstop checks pen mode this often, so it ends at most this long
/// after `ink::PEN_TIMEOUT_MS`.
const BACKSTOP_TICK: Duration = Duration::from_millis(500);
/// How long the bar stays up as "Copied" after a session.
pub const COPIED_VISIBLE: Duration = Duration::from_secs(8);

mod panel {
    tauri_nspanel::tauri_panel! {
        panel!(AnnotatePanel {
            config: {
                can_become_key_window: false,
                can_become_main_window: false,
                is_floating_panel: true
            }
        })
    }
}

struct Live {
    clock: Instant,
    started_at_ms: i64,
    ink: Ink,
}

impl Live {
    fn now(&self) -> u64 {
        self.clock.elapsed().as_millis() as u64
    }
}

/// The session's ink, while one records.
#[derive(Default)]
pub struct State(Mutex<Option<Live>>);

/// The pen states to put on screen, in the order the ink decided them.
/// One thread (`apply_pens`) owns click-through and the Esc registration,
/// so no two callers can interleave them and leave the overlay blocking
/// clicks with pen mode off.
struct PenQueue(Mutex<Sender<Pen>>);

/// Queues `pen`. Callers hold the `State` lock, so the queue order is the
/// order the ink changed.
fn queue_pen(app: &AppHandle, pen: Pen) {
    let queue = app.state::<PenQueue>();
    let sender = queue.0.lock().unwrap_or_else(|e| e.into_inner());
    if sender.send(pen).is_err() {
        log::warn!("[annotate] pen thread gone");
    }
}

/// The package that just finished, while the bar shows "Copied". `id` tells
/// the hide timer whether a newer one replaced it.
struct Copied {
    id: u64,
    contact_sheet: Option<PathBuf>,
}

#[derive(Default)]
struct CopiedState(Mutex<Option<Copied>>);

/// When the shortcut started the session that is still starting: the bar is
/// already up, waiting for the microphone and the capture.
#[derive(Default)]
struct StartingState(Mutex<Option<Instant>>);

fn with_starting<T>(app: &AppHandle, f: impl FnOnce(&mut Option<Instant>) -> T) -> T {
    let state = app.state::<StartingState>();
    let mut starting = state.0.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut starting)
}

fn with_copied<T>(app: &AppHandle, f: impl FnOnce(&mut Option<Copied>) -> T) -> T {
    let state = app.state::<CopiedState>();
    let mut copied = state.0.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut copied)
}

fn with_live<T>(app: &AppHandle, f: impl FnOnce(&mut Live) -> T) -> Option<T> {
    let state = app.state::<State>();
    let mut live = state.0.lock().unwrap_or_else(|e| e.into_inner());
    live.as_mut().map(f)
}

/// What the floating bar shows.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BarState {
    pub recording: bool,
    /// Unix ms of the start tap, for the bar's own timer.
    pub started_at_ms: i64,
    pub pen: bool,
    /// The shortcut was pressed and the capture is still starting.
    pub starting: bool,
    /// The session finished and its summary is on the clipboard.
    pub copied: bool,
    /// The finished session has a contact sheet to copy.
    pub has_image: bool,
}

pub fn bar_state(app: &AppHandle) -> BarState {
    let idle = BarState {
        recording: false,
        started_at_ms: 0,
        pen: false,
        starting: false,
        copied: false,
        has_image: false,
    };
    with_live(app, |live| BarState {
        recording: true,
        started_at_ms: live.started_at_ms,
        pen: !live.ink.pen().click_through(),
        ..idle.clone()
    })
    .or_else(|| {
        with_starting(app, |starting| starting.is_some()).then(|| BarState {
            starting: true,
            ..idle.clone()
        })
    })
    .or_else(|| {
        with_copied(app, |copied| {
            copied.as_ref().map(|copied| BarState {
                copied: true,
                has_image: copied.contact_sheet.is_some(),
                ..idle.clone()
            })
        })
    })
    .unwrap_or(idle)
}

/// A session's package is on the clipboard: bring the bar back as "Copied"
/// for `COPIED_VISIBLE`, where it last stood.
pub fn show_copied(app: &AppHandle, contact_sheet: Option<PathBuf>) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    with_copied(app, |copied| *copied = Some(Copied { id, contact_sheet }));
    emit_bar(app);
    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        if let Ok(panel) = handle.get_webview_panel(BAR) {
            crate::overlay::join_all_spaces(panel.as_panel());
            panel.order_front_regardless();
        }
    }) {
        log::warn!("[annotate] show copied: {error}");
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(COPIED_VISIBLE);
        if with_copied(&app, |copied| copied.as_ref().is_some_and(|c| c.id == id)) {
            dismiss(&app);
        }
    });
}

pub fn copied_contact_sheet(app: &AppHandle) -> Option<PathBuf> {
    with_copied(app, |copied| {
        copied
            .as_ref()
            .and_then(|copied| copied.contact_sheet.clone())
    })
}

/// Hides the "Copied" bar. Never hides the bar of a session that records.
pub fn dismiss(app: &AppHandle) {
    with_copied(app, |copied| *copied = None);
    if with_live(app, |_| ()).is_some() {
        return;
    }
    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        if let Ok(panel) = handle.get_webview_panel(BAR) {
            panel.hide();
        }
    }) {
        log::warn!("[annotate] hide copied: {error}");
    }
    emit_bar(app);
}

pub fn escape() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}

/// Creates both panels, hidden. Called once from `setup`, on the main thread,
/// which is where the spike converted its window.
pub fn setup(app: &App) -> tauri::Result<()> {
    app.manage(State::default());
    let (pens, queued) = mpsc::channel();
    app.manage(PenQueue(Mutex::new(pens)));
    let handle = app.handle().clone();
    std::thread::spawn(move || apply_pens(handle, queued));
    app.manage(CopiedState::default());
    app.manage(StartingState::default());
    let handle = app.handle();
    let levels = [
        (INK, "ink", crate::overlay::LEVEL - 1),
        (BAR, "annotate-bar", crate::overlay::LEVEL),
    ];
    for (label, query, level) in levels {
        let window = WebviewWindowBuilder::new(
            handle,
            label,
            WebviewUrl::App(format!("index.html?window={query}").into()),
        )
        .title("dictate Annotate")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .focused(false)
        .focusable(false)
        .accept_first_mouse(true)
        .visible(false)
        .build()?;
        let panel = window.to_panel::<panel::AnnotatePanel>()?;
        panel
            .add_style_mask(objc2_app_kit::NSWindowStyleMask::NonactivatingPanel)
            .map_err(|e| tauri::Error::Io(std::io::Error::other(format!("{e:?}"))))?;
        panel.set_level(level);
        panel.set_collection_behavior(crate::overlay::collection_behavior());
        panel.set_hides_on_deactivate(false);
        panel.set_transparent(true);
        panel.set_has_shadow(false);
        window.set_ignore_cursor_events(label == INK)?;
    }
    Ok(())
}

/// The shortcut started a session: show the bar on `display` at once, before
/// the microphone and the capture start, so the press has an answer. The
/// panel was built at launch, so this only moves it and orders it front.
pub fn show_starting(app: &AppHandle, display: &Display, pressed: Instant) {
    with_copied(app, |copied| *copied = None);
    with_starting(app, |starting| *starting = Some(pressed));
    emit_bar(app);
    let frame = ink::bar_frame(&display.frame, overlay_position(app));
    let handle = app.clone();
    let shown = app.run_on_main_thread(move || {
        show_panel(&handle, BAR, &frame);
        log::info!(
            "[annotate] timing shortcut_to_bar_shown_ms={}",
            pressed.elapsed().as_millis()
        );
    });
    if let Err(error) = shown {
        log::warn!("[annotate] show bar: {error}");
    }
}

/// The session never started: take the waiting bar down again.
pub fn cancel_starting(app: &AppHandle) {
    if with_starting(app, Option::take).is_none() {
        return;
    }
    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        if let Ok(panel) = handle.get_webview_panel(BAR) {
            panel.hide();
        }
    }) {
        log::warn!("[annotate] hide bar: {error}");
    }
    emit_bar(app);
}

fn overlay_position(app: &AppHandle) -> OverlayPosition {
    *app.state::<Mutex<OverlayPosition>>()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Moves panel `label` to `rect` (global top-left points) and orders it
/// front without activating dictate. Main thread only.
fn show_panel(app: &AppHandle, label: &str, rect: &crate::package::Rect) {
    let Ok(panel) = app.get_webview_panel(label) else {
        log::warn!("[annotate] {label} panel missing");
        return;
    };
    let primary = core_graphics::display::CGDisplay::main()
        .bounds()
        .size
        .height;
    let rect = ink::cocoa_frame(rect, primary);
    panel.as_panel().setFrame_display(
        objc2_foundation::NSRect::new(
            objc2_foundation::NSPoint::new(rect.x, rect.y),
            objc2_foundation::NSSize::new(rect.w, rect.h),
        ),
        true,
    );
    crate::overlay::join_all_spaces(panel.as_panel());
    panel.order_front_regardless();
}

/// A session started: cover `display` with the ink panel, put the bar on it,
/// and start the pointer poll.
pub fn begin(app: &AppHandle, display: Display, clock: Instant, started_at_ms: i64) {
    with_copied(app, |copied| *copied = None);
    let pressed = with_starting(app, Option::take);
    let frame = display.frame.clone();
    let position = overlay_position(app);
    *app.state::<State>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(Live {
        clock,
        started_at_ms,
        ink: Ink::new(display),
    });
    let handle = app.clone();
    let shown = app.run_on_main_thread(move || {
        show_panel(&handle, INK, &frame);
        show_panel(&handle, BAR, &ink::bar_frame(&frame, position));
    });
    if let Err(error) = shown {
        log::warn!("[annotate] show panels: {error}");
    }
    emit(app, INK, "ink-clear", ());
    emit_bar(app);
    if let Some(pressed) = pressed {
        log::info!(
            "[annotate] timing shortcut_to_bar_populated_ms={}",
            pressed.elapsed().as_millis()
        );
    }
    let app = app.clone();
    std::thread::spawn(move || poll(app, started_at_ms));
}

/// The session stopped: every stroke, with pen mode off, Esc released and
/// both panels hidden.
pub fn end(app: &AppHandle) -> Vec<Stroke> {
    let strokes = {
        let state = app.state::<State>();
        let mut live = state.0.lock().unwrap_or_else(|e| e.into_inner());
        let strokes = live.take().map(|live| {
            let now = live.now();
            live.ink.finish(now)
        });
        queue_pen(app, Pen::Off);
        strokes.unwrap_or_default()
    };
    let handle = app.clone();
    let hidden = app.run_on_main_thread(move || {
        for label in [INK, BAR] {
            if let Ok(panel) = handle.get_webview_panel(label) {
                panel.hide();
            }
        }
    });
    if let Err(error) = hidden {
        log::warn!("[annotate] hide panels: {error}");
    }
    emit(app, INK, "ink-clear", ());
    emit_bar(app);
    strokes
}

/// The Annotate shortcut went down while recording: open a provisional stroke.
pub fn pressed(app: &AppHandle) {
    let Some((x, y)) = pointer() else {
        log::warn!("[annotate] pointer unavailable");
        return;
    };
    if let Some(Some(point)) = with_live(app, |live| {
        let now = live.now();
        live.ink.press(now, x, y)
    }) {
        emit(app, INK, "ink-point", point);
    }
}

/// The Annotate shortcut came up while recording. True when it was a tap,
/// which stops the session.
pub fn released(app: &AppHandle) -> bool {
    match with_live(app, |live| {
        let now = live.now();
        live.ink.release(now)
    }) {
        Some(Release::Tap { id }) => {
            emit(app, INK, "ink-discard", id);
            true
        }
        Some(Release::Kept { id }) => {
            emit(app, INK, "ink-end", id);
            false
        }
        Some(Release::Shadowed { id }) => {
            emit(app, INK, "ink-discard", id);
            false
        }
        Some(Release::Nothing) | None => false,
    }
}

/// Mouse down on the ink panel. Only reaches us in pen mode, because the
/// panel is click-through otherwise; `Ink` checks again.
pub fn pen_down(app: &AppHandle) {
    let Some((x, y)) = pointer() else {
        return;
    };
    if let Some(Some(point)) = with_live(app, |live| {
        let now = live.now();
        live.ink.pen_down(now, x, y)
    }) {
        emit(app, INK, "ink-point", point);
    }
}

pub fn pen_up(app: &AppHandle) {
    if let Some(Some(id)) = with_live(app, |live| {
        let now = live.now();
        live.ink.pen_up(now)
    }) {
        emit(app, INK, "ink-end", id);
    }
}

/// The pen button, Esc, or the backstop's tick. The new state is queued for
/// `apply_pens` while the state lock is still held.
pub fn pen(app: &AppHandle, input: impl FnOnce(u64) -> PenInput) {
    let Some((before, after, closed)) = with_live(app, |live| {
        let now = live.now();
        let before = live.ink.pen();
        let (after, closed) = live.ink.set_pen(input(now), now);
        if after != before {
            queue_pen(app, after);
        }
        (before, after, closed)
    }) else {
        return;
    };
    if let Some(id) = closed {
        emit(app, INK, "ink-end", id);
    }
    if let Pen::On { since_ms } = after
        && before.click_through()
    {
        let app = app.clone();
        std::thread::spawn(move || backstop(app, since_ms));
    }
}

/// The only caller of the click-through and Esc calls. Each queued pen state
/// is compared with what is on screen, so a repeated or stale state changes
/// nothing. These calls wait on the main thread, which is why they run here
/// and not under the `State` lock.
fn apply_pens(app: AppHandle, queued: mpsc::Receiver<Pen>) {
    let mut shown = Pen::Off;
    for pen in queued {
        if pen.click_through() != shown.click_through()
            && let Some(window) = app.get_webview_window(INK)
            && let Err(error) = window.set_ignore_cursor_events(pen.click_through())
        {
            log::warn!("[annotate] click-through: {error}");
        }
        if pen.wants_escape() != shown.wants_escape() {
            let shortcuts = app.global_shortcut();
            let result = if pen.wants_escape() {
                shortcuts.register(escape())
            } else {
                shortcuts.unregister(escape())
            };
            if let Err(error) = result {
                log::warn!("[annotate] Esc: {error}");
            }
        }
        if pen.click_through() != shown.click_through() {
            log::info!(
                "[annotate] pen {}",
                if pen.click_through() { "off" } else { "on" }
            );
        }
        shown = pen;
        emit_bar(&app);
    }
}

/// Ends pen mode `ink::PEN_TIMEOUT_MS` after it started, unless something
/// else ended it first.
fn backstop(app: AppHandle, since_ms: u64) {
    loop {
        std::thread::sleep(BACKSTOP_TICK);
        let still_on = with_live(&app, |live| live.ink.pen() == Pen::On { since_ms });
        if still_on != Some(true) {
            return;
        }
        pen(&app, |now_ms| PenInput::Tick { now_ms });
    }
}

/// Samples the pointer at about 120 Hz while a stroke is open, and watches
/// Shift and the mouse button for Shift to draw, until this session ends.
fn poll(app: AppHandle, session: i64) {
    let mut shift = false;
    loop {
        std::thread::sleep(ink::POLL_INTERVAL);
        let (held, button) = shift_and_button().unwrap_or_default();
        if held != shift {
            shift = held;
            pen(&app, |_| PenInput::Shift { down: held });
        }
        let active = with_live(&app, |live| {
            (live.started_at_ms == session)
                .then(|| live.ink.drawing() || live.ink.pen() == Pen::Shift)
        });
        match active {
            Some(Some(true)) => {}
            Some(Some(false)) => continue,
            _ => return,
        }
        let Some((x, y)) = pointer() else {
            continue;
        };
        let (edge, points) = with_live(&app, |live| {
            let now = live.now();
            let edge = live.ink.shift_button(now, button, x, y);
            (edge, live.ink.sample(now, x, y))
        })
        .unwrap_or((ShiftButton::None, Vec::new()));
        match edge {
            ShiftButton::Opened(point) => emit(&app, INK, "ink-point", point),
            ShiftButton::Closed(id) => emit(&app, INK, "ink-end", id),
            ShiftButton::None => {}
        }
        for point in points {
            emit(&app, INK, "ink-point", point);
        }
    }
}

fn emit_bar(app: &AppHandle) {
    emit(app, BAR, "annotate-bar", bar_state(app));
}

fn emit(app: &AppHandle, label: &str, event: &str, payload: impl Serialize + Clone) {
    if let Err(error) = app.emit_to(label, event, payload) {
        log::warn!("[annotate] {event}: {error}");
    }
}
