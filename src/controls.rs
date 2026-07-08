//! System media controls: MPRIS on Linux, System Media Transport Controls on
//! Windows, and the Now Playing / MediaRemote center on macOS. This also gives
//! us hardware/keyboard media-key support on every platform, courtesy of
//! `souvlaki`.

use anyhow::{anyhow, Result};
use souvlaki::{MediaControlEvent, MediaControls, PlatformConfig};
use tao::window::Window;

use crate::config::{APP_DISPLAY_NAME, DBUS_NAME};

/// Create the platform media controls bound to `window` (the window handle is
/// required for the Windows SMTC backend; ignored elsewhere) and forward every
/// control event to `handler`.
pub fn build<F>(window: &Window, handler: F) -> Result<MediaControls>
where
    F: Fn(MediaControlEvent) + Send + 'static,
{
    #[cfg(target_os = "windows")]
    let hwnd = {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => Some(h.hwnd.get() as *mut std::ffi::c_void),
            _ => None,
        }
    };
    #[cfg(not(target_os = "windows"))]
    let hwnd = {
        let _ = window;
        None
    };

    let config = PlatformConfig {
        display_name: APP_DISPLAY_NAME,
        dbus_name: DBUS_NAME,
        hwnd,
    };

    let mut controls =
        MediaControls::new(config).map_err(|e| anyhow!("creating media controls: {e:?}"))?;
    controls
        .attach(handler)
        .map_err(|e| anyhow!("attaching media control handler: {e:?}"))?;
    Ok(controls)
}
