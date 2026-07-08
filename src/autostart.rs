//! Cross-platform "start on login" registration, backed by the `auto-launch`
//! crate (Windows registry Run key, macOS LaunchAgent, XDG autostart on Linux).

use anyhow::{anyhow, Result};
use auto_launch::{AutoLaunch, AutoLaunchBuilder};

use crate::config::APP_DISPLAY_NAME;

/// Build an [`AutoLaunch`] handle pointing at the currently running executable.
fn handle() -> Result<AutoLaunch> {
    let exe = std::env::current_exe()?;
    let exe = exe.to_string_lossy().to_string();

    let mut builder = AutoLaunchBuilder::new();
    builder.set_app_name(APP_DISPLAY_NAME).set_app_path(&exe);
    // On macOS a LaunchAgent plist is friendlier than an AppleScript login item.
    #[cfg(target_os = "macos")]
    builder.set_use_launch_agent(true);

    builder
        .build()
        .map_err(|e| anyhow!("auto-launch init failed: {e}"))
}

/// Whether the app is currently registered to start on login.
pub fn is_enabled() -> bool {
    handle()
        .and_then(|h| h.is_enabled().map_err(|e| anyhow!(e)))
        .unwrap_or(false)
}

/// Enable or disable start-on-login.
pub fn set(enabled: bool) -> Result<()> {
    let h = handle()?;
    if enabled {
        h.enable().map_err(|e| anyhow!("enabling autostart: {e}"))
    } else {
        h.disable().map_err(|e| anyhow!("disabling autostart: {e}"))
    }
}
