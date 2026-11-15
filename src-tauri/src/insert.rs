use crate::problem::Problem;
use tauri::AppHandle;

pub fn accessibility_needed() -> Problem {
    Problem::new(
        "Accessibility access needed",
        "Allow dictate in System Settings → Privacy & Security → Accessibility.",
    )
}

pub fn paste(_app: &AppHandle, text: &str) -> Result<(), Problem> {
    #[cfg(target_os = "macos")]
    return objc2::rc::autoreleasepool(|_| macos::paste(text));
    #[cfg(not(target_os = "macos"))]
    Err(Problem::new(
        "Paste unavailable",
        "dictate can paste only on macOS.",
    ))
}

#[cfg(target_os = "macos")]
mod macos {
    use crate::problem::Problem;
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

    pub(super) fn paste(text: &str) -> Result<(), Problem> {
        let _guard = PASTE
            .try_lock()
            .map_err(|_| Problem::new("Still pasting", "Wait a moment, then dictate again."))?;
        // SAFETY: AXIsProcessTrusted has no arguments or caller preconditions.
        if !unsafe { AXIsProcessTrusted() } {
            return Err(super::accessibility_needed());
        }
        if text.trim().is_empty() {
            return Ok(());
        }
        let source = CGEventSource::new(CGEventSourceStateID::Private)
            .map_err(|_| paste_failed("Could not create keyboard event source"))?;
        let down = CGEvent::new_keyboard_event(source.clone(), 9, true)
            .map_err(|_| paste_failed("Could not create paste key-down"))?;
        let up = CGEvent::new_keyboard_event(source, 9, false)
            .map_err(|_| paste_failed("Could not create paste key-up"))?;
        down.set_flags(CGEventFlags::CGEventFlagCommand);
        up.set_flags(CGEventFlags::CGEventFlagCommand);

        paste_to_board(&NSPasteboard::generalPasteboard(), text, || {
            down.post(CGEventTapLocation::HID);
            up.post(CGEventTapLocation::HID);
        })
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
            println!("[insert] restore_delay_ms={}", posted.elapsed().as_millis());
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
