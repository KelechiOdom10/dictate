//! The app and display active when the user starts a take.

#[cfg(target_os = "macos")]
pub fn frontmost_app() -> Option<String> {
    objc2::rc::autoreleasepool(|_| {
        objc2_app_kit::NSWorkspace::sharedWorkspace()
            .frontmostApplication()?
            .localizedName()
            .map(|name| name.to_string())
    })
}

/// Name and bundle id of the frontmost app, for an Annotate package.
#[cfg(target_os = "macos")]
pub fn frontmost_app_id() -> Option<crate::package::App> {
    objc2::rc::autoreleasepool(|_| {
        let app = objc2_app_kit::NSWorkspace::sharedWorkspace().frontmostApplication()?;
        Some(crate::package::App {
            name: app.localizedName()?.to_string(),
            bundle_id: app.bundleIdentifier()?.to_string(),
        })
    })
}

#[cfg(not(target_os = "macos"))]
pub fn frontmost_app() -> Option<String> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn frontmost_app_id() -> Option<crate::package::App> {
    None
}

#[cfg(target_os = "macos")]
pub fn editable_target() -> bool {
    use core_foundation::{
        base::{CFType, CFTypeRef, TCFType},
        string::{CFString, CFStringRef},
    };
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXUIElementCreateSystemWide() -> CFTypeRef;
        fn AXUIElementSetMessagingTimeout(element: CFTypeRef, timeout: f32) -> i32;
        fn AXUIElementCopyAttributeValue(
            element: CFTypeRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> i32;
        fn AXUIElementIsAttributeSettable(
            element: CFTypeRef,
            attribute: CFStringRef,
            settable: *mut bool,
        ) -> i32;
    }
    // SAFETY: Create returns an owned AX element. Copy returns an owned CF value
    // only on success. Both are released by CFType; all calls are bounded.
    unsafe {
        let system = CFType::wrap_under_create_rule(AXUIElementCreateSystemWide());
        AXUIElementSetMessagingTimeout(system.as_CFTypeRef(), 0.1);
        let mut focused = std::ptr::null();
        if AXUIElementCopyAttributeValue(
            system.as_CFTypeRef(),
            CFString::new("AXFocusedUIElement").as_concrete_TypeRef(),
            &mut focused,
        ) != 0
            || focused.is_null()
        {
            return false;
        }
        let focused = CFType::wrap_under_create_rule(focused);
        AXUIElementSetMessagingTimeout(focused.as_CFTypeRef(), 0.1);
        let mut role = std::ptr::null();
        if AXUIElementCopyAttributeValue(
            focused.as_CFTypeRef(),
            CFString::new("AXRole").as_concrete_TypeRef(),
            &mut role,
        ) != 0
            || role.is_null()
        {
            return false;
        }
        let role = CFType::wrap_under_create_rule(role);
        let Some(role) = role.downcast::<CFString>() else {
            return false;
        };
        if !matches!(
            role.to_string().as_str(),
            "AXTextField" | "AXTextArea" | "AXComboBox"
        ) {
            return false;
        }
        let mut writable = false;
        AXUIElementIsAttributeSettable(
            focused.as_CFTypeRef(),
            CFString::new("AXValue").as_concrete_TypeRef(),
            &mut writable,
        ) == 0
            && writable
    }
}

#[cfg(not(target_os = "macos"))]
pub fn editable_target() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn focused_window_center() -> Option<(f64, f64)> {
    None
}

/// How long one Accessibility call may wait on the focused app. The default is
/// about 6 s, which would stall the hotkey on a hung app; past this we fall
/// back to the main display.
#[cfg(target_os = "macos")]
const AX_TIMEOUT_SECONDS: f32 = 0.1;

#[cfg(target_os = "macos")]
pub fn focused_window_center() -> Option<(f64, f64)> {
    use core_foundation::{
        base::{CFType, CFTypeID, CFTypeRef, TCFType},
        string::{CFString, CFStringRef},
    };
    use core_graphics::geometry::{CGPoint, CGSize};
    use std::ffi::c_void;
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXUIElementCreateSystemWide() -> CFTypeRef;
        fn AXUIElementSetMessagingTimeout(element: CFTypeRef, timeout: f32) -> i32;
        fn AXUIElementCopyAttributeValue(
            element: CFTypeRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> i32;
        fn AXValueGetTypeID() -> CFTypeID;
        fn AXValueGetType(value: CFTypeRef) -> u32;
        fn AXValueGetValue(value: CFTypeRef, kind: u32, output: *mut c_void) -> bool;
    }
    fn attribute(element: &CFType, name: &str) -> Option<CFType> {
        let mut value = std::ptr::null();
        // SAFETY: element and key remain alive for the call; successful Copy returns an owned CF object.
        let result = unsafe {
            AXUIElementCopyAttributeValue(
                element.as_CFTypeRef(),
                CFString::new(name).as_concrete_TypeRef(),
                &mut value,
            )
        };
        if result != 0 || value.is_null() {
            return None;
        }
        // SAFETY: checked successful Copy ownership above.
        Some(unsafe { CFType::wrap_under_create_rule(value) })
    }
    // SAFETY: CreateSystemWide takes no arguments and returns an owned accessibility element.
    let system = unsafe { CFType::wrap_under_create_rule(AXUIElementCreateSystemWide()) };
    let limit = |element: &CFType| {
        // SAFETY: element is a live AXUIElement for the duration of the call.
        unsafe { AXUIElementSetMessagingTimeout(element.as_CFTypeRef(), AX_TIMEOUT_SECONDS) }
    };
    limit(&system);
    let app = attribute(&system, "AXFocusedApplication")?;
    limit(&app);
    let window = attribute(&app, "AXFocusedWindow")?;
    let position = attribute(&window, "AXPosition")?;
    let size = attribute(&window, "AXSize")?;
    let mut point = CGPoint::new(0.0, 0.0);
    let mut dimensions = CGSize::new(0.0, 0.0);
    // SAFETY: AXValue type checks establish the output layouts: CGPoint=1 and CGSize=2.
    let valid = unsafe {
        position.type_of() == AXValueGetTypeID()
            && size.type_of() == AXValueGetTypeID()
            && AXValueGetType(position.as_CFTypeRef()) == 1
            && AXValueGetType(size.as_CFTypeRef()) == 2
            && AXValueGetValue(
                position.as_CFTypeRef(),
                1,
                (&mut point as *mut CGPoint).cast(),
            )
            && AXValueGetValue(
                size.as_CFTypeRef(),
                2,
                (&mut dimensions as *mut CGSize).cast(),
            )
    };
    if !valid {
        return None;
    }
    window_center((point.x, point.y), (dimensions.width, dimensions.height))
}

fn window_center(position: (f64, f64), size: (f64, f64)) -> Option<(f64, f64)> {
    let (x, y) = position;
    let (width, height) = size;
    let center = (x + width / 2.0, y + height / 2.0);
    (x.is_finite()
        && y.is_finite()
        && width.is_finite()
        && height.is_finite()
        && width > 0.0
        && height > 0.0
        && center.0.is_finite()
        && center.1.is_finite())
    .then_some(center)
}

#[cfg(not(target_os = "macos"))]
pub fn pointer() -> Option<(f64, f64)> {
    None
}

/// Global pointer position in display points, top-left origin.
#[cfg(target_os = "macos")]
pub fn pointer() -> Option<(f64, f64)> {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let point = CGEvent::new(source).ok()?.location();
    Some((point.x, point.y))
}

/// Whether Shift is the only modifier held, and whether the left button is
/// down, from the session's combined input state.
#[cfg(target_os = "macos")]
pub fn shift_and_button() -> Option<(bool, bool)> {
    use core_graphics::event::{CGEventFlags, CGMouseButton};
    use core_graphics::event_source::CGEventSourceStateID;
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceButtonState(state: CGEventSourceStateID, button: CGMouseButton) -> bool;
        /// The live modifier flags of the session. A freshly created event has
        /// no flags of its own, so reading one always said "nothing held".
        fn CGEventSourceFlagsState(state: CGEventSourceStateID) -> u64;
    }
    let state = CGEventSourceStateID::CombinedSessionState;
    let flagged = unsafe { CGEventSourceFlagsState(state) };
    let modifiers = (CGEventFlags::CGEventFlagShift
        | CGEventFlags::CGEventFlagCommand
        | CGEventFlags::CGEventFlagControl
        | CGEventFlags::CGEventFlagAlternate)
        .bits();
    let button = unsafe { CGEventSourceButtonState(state, CGMouseButton::Left) };
    Some((
        flagged & modifiers == CGEventFlags::CGEventFlagShift.bits(),
        button,
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn shift_and_button() -> Option<(bool, bool)> {
    None
}

/// The display holding the focused window's centre, else the pointer.
fn selected_display(
    displays: &[crate::package::Display],
    focused: Option<(f64, f64)>,
    pointer: Option<(f64, f64)>,
) -> Option<&crate::package::Display> {
    [focused, pointer]
        .into_iter()
        .flatten()
        .find_map(|(x, y)| crate::ink::display_at(displays, x, y))
}

/// The display a take or session belongs to, falling back to the main display.
#[cfg(target_os = "macos")]
pub fn active_display() -> crate::package::Display {
    use core_graphics::display::CGDisplay;
    fn describe(display: CGDisplay) -> crate::package::Display {
        let bounds = display.bounds();
        crate::package::Display {
            id: display.id,
            frame: crate::package::Rect {
                x: bounds.origin.x,
                y: bounds.origin.y,
                w: bounds.size.width,
                h: bounds.size.height,
            },
            scale: if bounds.size.width > 0.0 {
                display.pixels_wide() as f64 / bounds.size.width
            } else {
                1.0
            },
        }
    }
    let displays: Vec<_> = CGDisplay::active_displays()
        .unwrap_or_default()
        .into_iter()
        .map(|id| describe(CGDisplay::new(id)))
        .collect();
    selected_display(&displays, focused_window_center(), pointer())
        .cloned()
        .unwrap_or_else(|| describe(CGDisplay::main()))
}

#[cfg(test)]
mod tests {
    use super::{selected_display, shift_and_button, window_center};
    use crate::package::{Display, Rect};

    /// Why `shift_and_button` reads the session instead of an event: a new
    /// event carries no modifier flags, so `CGEvent::new(..).get_flags()`
    /// answered "no modifiers held" forever and Shift-draw never started
    /// natively.
    ///
    /// Only the modifier bits are asserted. A new event is not flag-empty in
    /// general: on this machine its flags are exactly `CGEventFlagNonCoalesced`,
    /// which is not a modifier, so comparing against an empty set failed for a
    /// reason that has nothing to do with the bug guarded here. The masking is
    /// not a loosening — a modifier physically held while the suite runs still
    /// lands in the mask and still fails this test, which is the behaviour
    /// being checked. That makes it the one test here needing an idle keyboard.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_new_event_carries_no_modifier_flags() {
        use core_graphics::event::{CGEvent, CGEventFlags};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).unwrap();
        let event = CGEvent::new(source).unwrap();
        let modifiers = CGEventFlags::CGEventFlagAlphaShift
            | CGEventFlags::CGEventFlagShift
            | CGEventFlags::CGEventFlagControl
            | CGEventFlags::CGEventFlagAlternate
            | CGEventFlags::CGEventFlagCommand
            | CGEventFlags::CGEventFlagSecondaryFn;
        assert_eq!(event.get_flags() & modifiers, CGEventFlags::empty());
    }

    /// The session read itself must work with nothing held, and must not panic.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_session_flags_can_be_read() {
        assert!(shift_and_button().is_some());
    }

    fn display(id: u32, x: f64, y: f64, w: f64, h: f64) -> Display {
        Display {
            id,
            frame: Rect { x, y, w, h },
            scale: 2.0,
        }
    }

    fn displays() -> Vec<Display> {
        vec![
            display(1, 0.0, 0.0, 1512.0, 982.0),
            display(2, -1920.0, -1080.0, 1920.0, 1080.0),
        ]
    }

    #[test]
    fn the_focused_window_wins_over_the_pointer() {
        let displays = displays();
        let chosen = selected_display(&displays, Some((-900.0, -500.0)), Some((10.0, 10.0)));
        assert_eq!(chosen.unwrap().id, 2);
    }

    #[test]
    fn the_pointer_decides_when_no_window_is_focused_or_it_is_off_screen() {
        let displays = displays();
        assert_eq!(
            selected_display(&displays, None, Some((-1.0, -1.0)))
                .unwrap()
                .id,
            2
        );
        assert_eq!(
            selected_display(&displays, Some((5000.0, 5000.0)), Some((100.0, 100.0)))
                .unwrap()
                .id,
            1
        );
    }

    #[test]
    fn edges_belong_to_the_next_display_and_nowhere_means_no_choice() {
        let displays = displays();
        assert_eq!(
            selected_display(&displays, Some((0.0, 0.0)), None)
                .unwrap()
                .id,
            1
        );
        assert!(selected_display(&displays, Some((1512.0, 10.0)), None).is_none());
        assert!(selected_display(&displays, None, None).is_none());
        assert!(selected_display(&[], Some((0.0, 0.0)), Some((0.0, 0.0))).is_none());
    }

    #[test]
    fn a_window_centre_needs_a_real_size() {
        assert_eq!(
            window_center((-100.0, 20.0), (200.0, 40.0)),
            Some((0.0, 40.0))
        );
        assert!(window_center((0.0, 0.0), (0.0, 40.0)).is_none());
        assert!(window_center((f64::NAN, 0.0), (10.0, 10.0)).is_none());
        assert!(window_center((0.0, 0.0), (f64::INFINITY, 10.0)).is_none());
    }
}
