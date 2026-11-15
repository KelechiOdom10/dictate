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
