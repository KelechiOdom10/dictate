use tauri::AppHandle;

pub fn paste(_app: &AppHandle, text: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return objc2::rc::autoreleasepool(|_| macos::paste(text));
    #[cfg(not(target_os = "macos"))]
    Err("Focused-app paste is supported only on macOS".into())
}

#[cfg(target_os = "macos")]
mod macos {
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

    pub(super) fn paste(text: &str) -> Result<(), String> {
        let _guard = PASTE
            .try_lock()
            .map_err(|_| "Another paste is in progress")?;
        // SAFETY: AXIsProcessTrusted has no arguments or caller preconditions.
        if !unsafe { AXIsProcessTrusted() } {
            return Err(
                "Allow dictate in System Settings → Privacy & Security → Accessibility, then retry"
                    .into(),
            );
        }
        if text.trim().is_empty() {
            return Ok(());
        }
        let source = CGEventSource::new(CGEventSourceStateID::Private)
            .map_err(|_| "Could not create keyboard event source")?;
        let down = CGEvent::new_keyboard_event(source.clone(), 9, true)
            .map_err(|_| "Could not create paste key-down")?;
        let up = CGEvent::new_keyboard_event(source, 9, false)
            .map_err(|_| "Could not create paste key-up")?;
        down.set_flags(CGEventFlags::CGEventFlagCommand);
        up.set_flags(CGEventFlags::CGEventFlagCommand);

        paste_to_board(&NSPasteboard::generalPasteboard(), text, || {
            down.post(CGEventTapLocation::HID);
            up.post(CGEventTapLocation::HID);
        })
    }

    fn paste_to_board(board: &NSPasteboard, text: &str, send: impl FnOnce()) -> Result<(), String> {
        let original_count = board.changeCount();
        let mut saved: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = Vec::new();
        if let Some(items) = board.pasteboardItems() {
            for item in items.iter() {
                let copy = NSPasteboardItem::new();
                for kind in item.types().iter() {
                    let data = item
                        .dataForType(&kind)
                        .ok_or("Could not save a clipboard format; clipboard left unchanged")?;
                    if !copy.setData_forType(&data, &kind) {
                        return Err(
                            "Could not save clipboard contents; clipboard left unchanged".into(),
                        );
                    }
                }
                saved.push(ProtocolObject::from_retained(copy));
            }
        }
        if board.changeCount() != original_count {
            return Err("Clipboard changed while saving it; retry dictation".into());
        }
        let transcript = NSPasteboardItem::new();
        // SAFETY: AppKit exports this immutable pasteboard type constant.
        if !transcript
            .setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString })
        {
            return Err("Could not prepare transcript for the clipboard".into());
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
            return Err("Clipboard changed during paste; kept the newer clipboard contents".into());
        }
        board.clearContents();
        if !saved.is_empty() && !board.writeObjects(&NSArray::from_retained_slice(&saved)) {
            return Err("Paste attempted but could not restore the previous clipboard".into());
        }
        if !written {
            return Err(
                "Could not put transcript on clipboard; previous clipboard restored".into(),
            );
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
            assert!(result.unwrap_err().contains("kept the newer clipboard"));
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
