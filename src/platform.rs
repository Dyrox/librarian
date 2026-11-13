//! Small OS integrations.

/// The OS appearance, read directly (iced pins the window's own appearance to
/// the app theme, so the window stops receiving change events).
#[cfg(target_os = "macos")]
pub fn system_dark() -> Option<bool> {
    use objc2_foundation::{NSString, NSUserDefaults};
    let defaults = NSUserDefaults::standardUserDefaults();
    let style = defaults.stringForKey(&NSString::from_str("AppleInterfaceStyle"));
    Some(style.is_some_and(|s| s.to_string().eq_ignore_ascii_case("dark")))
}

#[cfg(not(target_os = "macos"))]
pub fn system_dark() -> Option<bool> {
    // Windows/Linux: rely on iced's theme-change events.
    None
}
