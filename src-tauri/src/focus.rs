//! Which app the user was in when they started a take.

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
