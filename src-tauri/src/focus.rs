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

#[cfg(not(target_os = "macos"))]
pub fn frontmost_app() -> Option<String> {
    None
}
