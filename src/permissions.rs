use std::process::Command;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PermissionState {
    pub input_monitoring: bool,
    pub accessibility: bool,
}

#[cfg(target_os = "macos")]
pub fn detect_permissions() -> PermissionState {
    let accessibility = macos_accessibility_client::accessibility::application_is_trusted();
    let input_monitoring = has_event_tap_access();
    PermissionState {
        input_monitoring,
        accessibility,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn detect_permissions() -> PermissionState {
    PermissionState {
        input_monitoring: false,
        accessibility: false,
    }
}

#[cfg(target_os = "macos")]
pub fn prompt_accessibility_if_needed() {
    if !macos_accessibility_client::accessibility::application_is_trusted() {
        let _ = macos_accessibility_client::accessibility::application_is_trusted_with_prompt();
    }
}

#[cfg(not(target_os = "macos"))]
pub fn prompt_accessibility_if_needed() {}

pub fn open_permissions_settings() -> std::io::Result<()> {
    Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .status()?;

    Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
        .status()?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn has_event_tap_access() -> bool {
    use core_graphics::event::{
        CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
        CallbackResult,
    };

    let session_ok = CGEventTap::new(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        vec![CGEventType::KeyDown],
        |_proxy, _event_type, _event| CallbackResult::Keep,
    )
    .is_ok();

    if session_ok {
        return true;
    }

    CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        vec![CGEventType::KeyDown],
        |_proxy, _event_type, _event| CallbackResult::Keep,
    )
    .is_ok()
}
