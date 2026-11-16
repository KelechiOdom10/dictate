//! Annotate's copy: the summary as text, or the contact sheet as a PNG.
//! Each is its own single-type item. A mixed text-and-image item lets each
//! app pick one, so one app would paste only the image and another only the
//! text (design doc §5). Unlike dictation's paste, nothing is restored: the
//! user pastes this later.

use std::path::Path;

use objc2_app_kit::{NSPasteboard, NSPasteboardTypePNG, NSPasteboardTypeString};
use objc2_foundation::{NSData, NSString};

pub fn copy_text(text: &str) -> Result<(), String> {
    let board = NSPasteboard::generalPasteboard();
    board.clearContents();
    // SAFETY: AppKit exports this immutable pasteboard type constant.
    let kind = unsafe { NSPasteboardTypeString };
    board
        .setString_forType(&NSString::from_str(text), kind)
        .then_some(())
        .ok_or_else(|| "The clipboard refused the text".to_string())
}

pub fn copy_png(path: &Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Read {}: {e}", path.display()))?;
    let data = NSData::with_bytes(&bytes);
    let board = NSPasteboard::generalPasteboard();
    board.clearContents();
    // SAFETY: AppKit exports this immutable pasteboard type constant.
    let kind = unsafe { NSPasteboardTypePNG };
    board
        .setData_forType(Some(&data), kind)
        .then_some(())
        .ok_or_else(|| "The clipboard refused the image".to_string())
}
