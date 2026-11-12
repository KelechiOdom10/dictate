//! Insertion into the focused application.
//!
//! The mechanism is not text injection, it is ⌘V. You set the clipboard, post a
//! synthetic Cmd+V, and restore whatever was on the clipboard 300 ms later.
//!
//! That is what nearly every dictation tool actually does, including the one you
//! are replacing, and it is not a shortcut: `AXUIElement` set-value fails on
//! Electron apps, web content in some browsers, and anything with a custom text
//! view. Paste fails only where a text field refuses focus, which is a much
//! shorter list.
//!
//! Keep the clipboard-restore behaviour. Without it you silently destroy whatever
//! the user copied before dictating, and they will not notice until they paste
//! and get their own dictation back. That bug is the reason people distrust
//! dictation tools.

use tauri::AppHandle;

/// Put `text` into whatever app currently has focus.
///
/// TODO(you):
/// - read the current clipboard text via the clipboard-manager plugin and hold it
/// - write `text` to the clipboard
/// - post Cmd+V. Either `enigo` (fast, correct, and worth reading its source
///   afterwards to see what it wraps) or raw `core-graphics` `CGEventPost` for
///   the hand-rolled version — see the note below
/// - sleep ~300 ms, then restore the previous clipboard contents
///
/// macOS will refuse the keystroke until the app is trusted for Accessibility.
/// TCC does not prompt for a synthetic event from an unsigned dev build, it just
/// silently drops it. If nothing appears, that is the first thing to check, not
/// the last.
///
/// Spike now, hand-roll later: `enigo` gets you a working end-to-end pipeline on
/// day one. The hand-rolled `CGEventCreateKeyboardEvent` + `CGEventPost` version
/// is the week-2 exercise, and the point of doing both is that you will then know
/// exactly what `enigo` decided about event source state and flags that you would
/// have got wrong.
pub fn paste(_app: &AppHandle, _text: &str) -> Result<(), String> {
    Err("insert::paste not implemented".into())
}
