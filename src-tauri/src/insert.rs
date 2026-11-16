use crate::problem::Problem;
use crate::settings::{PasteMethod, Settings};
use tauri::AppHandle;

pub fn accessibility_needed() -> Problem {
    Problem::new(
        "Accessibility access needed",
        "Allow dictate in System Settings → Privacy & Security → Accessibility.",
    )
}

/// How a finished transcript reaches the user, read from Output settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Delivery {
    pub method: PasteMethod,
    pub restore_clipboard: bool,
}

impl Delivery {
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            method: settings.paste_method,
            restore_clipboard: settings.restore_clipboard,
        }
    }
}

/// Applies the Output text options to a trimmed transcript.
pub fn shape(text: &str, capitalize_first: bool, trailing_space: bool) -> String {
    let mut shaped = String::with_capacity(text.len() + 1);
    let mut capitalize = capitalize_first;
    for c in text.chars() {
        if capitalize && c.is_alphabetic() {
            shaped.extend(c.to_uppercase());
            capitalize = false;
        } else {
            if capitalize && c.is_alphanumeric() {
                capitalize = false;
            }
            shaped.push(c);
        }
    }
    if trailing_space && !shaped.is_empty() && !shaped.ends_with(char::is_whitespace) {
        shaped.push(' ');
    }
    shaped
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    Pasted,
    Copied,
}

pub fn paste(_app: &AppHandle, text: &str, delivery: Delivery) -> Result<Outcome, Problem> {
    #[cfg(target_os = "macos")]
    return objc2::rc::autoreleasepool(|_| macos::paste(text, delivery));
    #[cfg(not(target_os = "macos"))]
    Err(Problem::new(
        "Paste unavailable",
        "dictate can paste only on macOS.",
    ))
}

#[cfg(target_os = "macos")]
mod macos {
    use crate::problem::Problem;
    use crate::settings::PasteMethod;
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use objc2::{rc::Retained, runtime::ProtocolObject};
    use objc2_app_kit::{
        NSPasteboard, NSPasteboardItem, NSPasteboardTypeString, NSPasteboardWriting,
    };
    use objc2_foundation::{NSArray, NSString};
    use std::{
        sync::Mutex,
        thread,
        time::{Duration, Instant},
    };

    static PASTE: Mutex<()> = Mutex::new(());
    const RESTORE_DELAY: Duration = Duration::from_millis(300);

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }

    pub(super) fn paste(text: &str, delivery: super::Delivery) -> Result<super::Outcome, Problem> {
        let _guard = PASTE
            .try_lock()
            .map_err(|_| Problem::new("Still pasting", "Wait a moment, then dictate again."))?;
        if text.trim().is_empty() {
            return Ok(super::Outcome::Copied);
        }
        if delivery.method == PasteMethod::CopyOnly || !crate::focus::editable_target() {
            copy_to_board(&NSPasteboard::generalPasteboard(), text)?;
            return Ok(super::Outcome::Copied);
        }
        // SAFETY: AXIsProcessTrusted has no arguments or caller preconditions.
        if !unsafe { AXIsProcessTrusted() } {
            return Err(super::accessibility_needed());
        }
        let source = CGEventSource::new(CGEventSourceStateID::Private)
            .map_err(|_| paste_failed("Could not create keyboard event source"))?;
        let down = CGEvent::new_keyboard_event(source.clone(), 9, true)
            .map_err(|_| paste_failed("Could not create paste key-down"))?;
        let up = CGEvent::new_keyboard_event(source, 9, false)
            .map_err(|_| paste_failed("Could not create paste key-up"))?;
        down.set_flags(CGEventFlags::CGEventFlagCommand);
        up.set_flags(CGEventFlags::CGEventFlagCommand);

        let send = || {
            down.post(CGEventTapLocation::HID);
            up.post(CGEventTapLocation::HID);
        };
        let board = NSPasteboard::generalPasteboard();
        if delivery.restore_clipboard {
            paste_to_board(&board, text, send).map(|()| super::Outcome::Pasted)
        } else {
            copy_to_board(&board, text)?;
            send();
            Ok(super::Outcome::Pasted)
        }
    }

    /// Leaves the transcript on the clipboard, replacing what was there.
    fn copy_to_board(board: &NSPasteboard, text: &str) -> Result<(), Problem> {
        board.clearContents();
        // SAFETY: AppKit exports this immutable pasteboard type constant.
        if board.setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString }) {
            Ok(())
        } else {
            Err(Problem::new(
                "Couldn’t copy",
                "Your words are in History. Copy them from there.",
            )
            .because("Could not put transcript on clipboard"))
        }
    }

    fn clipboard_unsaved(cause: &str) -> Problem {
        Problem::new(
            "Clipboard couldn’t be saved",
            "Nothing was pasted. Copy your words from History.",
        )
        .because(cause)
    }

    fn paste_failed(cause: impl std::fmt::Display) -> Problem {
        Problem::new(
            "Couldn’t paste",
            "Your words are in History. Copy them from there.",
        )
        .because(cause)
    }

    fn paste_to_board(
        board: &NSPasteboard,
        text: &str,
        send: impl FnOnce(),
    ) -> Result<(), Problem> {
        let original_count = board.changeCount();
        let mut saved: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = Vec::new();
        if let Some(items) = board.pasteboardItems() {
            for item in items.iter() {
                let copy = NSPasteboardItem::new();
                for kind in item.types().iter() {
                    let data = item
                        .dataForType(&kind)
                        .ok_or_else(|| clipboard_unsaved("Could not save a clipboard format"))?;
                    if !copy.setData_forType(&data, &kind) {
                        return Err(clipboard_unsaved("Could not save clipboard contents"));
                    }
                }
                saved.push(ProtocolObject::from_retained(copy));
            }
        }
        if board.changeCount() != original_count {
            return Err(clipboard_unsaved("Clipboard changed while saving it"));
        }
        let transcript = NSPasteboardItem::new();
        // SAFETY: AppKit exports this immutable pasteboard type constant.
        if !transcript
            .setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString })
        {
            return Err(paste_failed(
                "Could not prepare transcript for the clipboard",
            ));
        }
        let transcript: Retained<ProtocolObject<dyn NSPasteboardWriting>> =
            ProtocolObject::from_retained(transcript);
        board.clearContents();
        let written = board.writeObjects(&NSArray::from_retained_slice(&[transcript]));
        let transcript_count = board.changeCount();
        if written {
            send();
            let posted = Instant::now();
            thread::sleep(RESTORE_DELAY);
            log::info!("[insert] restore_delay_ms={}", posted.elapsed().as_millis());
        }
        if board.changeCount() != transcript_count {
            return Err(Problem::new(
                "Pasted, clipboard left as is",
                "Something new was copied during the paste, so dictate kept it.",
            ));
        }
        board.clearContents();
        if !saved.is_empty() && !board.writeObjects(&NSArray::from_retained_slice(&saved)) {
            return Err(Problem::new(
                "Pasted, clipboard not restored",
                "Copy what you had before again.",
            ));
        }
        if !written {
            return Err(paste_failed(
                "Could not put transcript on clipboard; previous clipboard restored",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use objc2_foundation::NSData;

        #[test]
        fn restores_multiple_items_and_binary_formats_after_transcript_is_read() {
            let board = NSPasteboard::pasteboardWithUniqueName();
            let first = NSPasteboardItem::new();
            let second = NSPasteboardItem::new();
            let kind = NSString::from_str("test.dictate.binary");
            let bytes = NSData::with_bytes(&[0, 255, 17, 128]);
            assert!(first.setData_forType(&bytes, &kind));
            let rich = NSString::from_str("public.rtf");
            assert!(first.setData_forType(&NSData::with_bytes(b"{\\rtf1 original}"), &rich));
            assert!(second.setData_forType(&NSData::with_bytes(b"second"), &kind));
            let first: Retained<ProtocolObject<dyn NSPasteboardWriting>> =
                ProtocolObject::from_retained(first);
            let second: Retained<ProtocolObject<dyn NSPasteboardWriting>> =
                ProtocolObject::from_retained(second);
            assert!(board.writeObjects(&NSArray::from_retained_slice(&[first, second])));
            paste_to_board(&board, "spoken words", || {
                // SAFETY: AppKit exports this immutable pasteboard type constant.
                assert_eq!(
                    board
                        .stringForType(unsafe { NSPasteboardTypeString })
                        .unwrap()
                        .to_string(),
                    "spoken words"
                );
            })
            .unwrap();
            let items = board.pasteboardItems().unwrap();
            assert_eq!(items.len(), 2);
            assert_eq!(
                items.objectAtIndex(0).dataForType(&rich).unwrap().to_vec(),
                b"{\\rtf1 original}"
            );
            assert_eq!(
                items.objectAtIndex(0).dataForType(&kind).unwrap().to_vec(),
                [0, 255, 17, 128]
            );
            assert_eq!(
                items.objectAtIndex(1).dataForType(&kind).unwrap().to_vec(),
                b"second"
            );
            board.clearContents();
        }

        #[test]
        fn copy_only_replaces_the_clipboard_with_the_transcript() {
            let board = NSPasteboard::pasteboardWithUniqueName();
            // SAFETY: AppKit exports this immutable pasteboard type constant.
            let string = unsafe { NSPasteboardTypeString };
            assert!(board.setString_forType(&NSString::from_str("before"), string));
            copy_to_board(&board, "spoken words ").unwrap();
            let items = board.pasteboardItems().unwrap();
            assert_eq!(items.len(), 1);
            assert_eq!(
                board.stringForType(string).unwrap().to_string(),
                "spoken words "
            );
            board.clearContents();
        }

        #[test]
        fn restores_empty_clipboard_and_preserves_a_new_copy_during_paste() {
            let board = NSPasteboard::pasteboardWithUniqueName();
            paste_to_board(&board, "spoken words", || {}).unwrap();
            assert!(board.pasteboardItems().is_none_or(|items| items.is_empty()));
            let result = paste_to_board(&board, "spoken words", || {
                board.clearContents();
                // SAFETY: AppKit exports this immutable pasteboard type constant.
                assert!(
                    board.setString_forType(&NSString::from_str("new copy"), unsafe {
                        NSPasteboardTypeString
                    })
                );
            });
            assert_eq!(result.unwrap_err().title, "Pasted, clipboard left as is");
            // SAFETY: AppKit exports this immutable pasteboard type constant.
            assert_eq!(
                board
                    .stringForType(unsafe { NSPasteboardTypeString })
                    .unwrap()
                    .to_string(),
                "new copy"
            );
            board.clearContents();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_capitalizes_the_first_letter_and_adds_one_trailing_space() {
        assert_eq!(shape("hello world", false, false), "hello world");
        assert_eq!(shape("hello world", true, false), "Hello world");
        assert_eq!(shape("hello world", false, true), "hello world ");
        assert_eq!(shape("hello world", true, true), "Hello world ");
        assert_eq!(shape("\"quoted\" text", true, false), "\"Quoted\" text");
        assert_eq!(shape("éclair", true, false), "Éclair");
        assert_eq!(shape("42 apples", true, false), "42 apples");
        assert_eq!(shape("Already.", true, true), "Already. ");
        assert_eq!(shape("ends with space ", false, true), "ends with space ");
        assert_eq!(shape("", true, true), "");
    }

    #[test]
    fn delivery_follows_output_settings() {
        let settings = Settings {
            paste_method: PasteMethod::CopyOnly,
            restore_clipboard: false,
            ..Settings::default()
        };
        assert_eq!(
            Delivery::from_settings(&settings),
            Delivery {
                method: PasteMethod::CopyOnly,
                restore_clipboard: false
            }
        );
        assert_eq!(
            Delivery::from_settings(&Settings::default()),
            Delivery {
                method: PasteMethod::Paste,
                restore_clipboard: true
            }
        );
    }
}
