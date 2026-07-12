//! Desktop "now playing" notifications, shown when the track changes.

use notify_rust::Notification;

use crate::config::{APP_DISPLAY_NAME, APP_ID};
use crate::util;

/// Show a desktop notification announcing the newly playing track.
///
/// Best-effort: any failure (e.g. no notification daemon running) is logged and
/// otherwise ignored — a missing notification must never disrupt playback.
pub fn song_changed(title: &str) {
    #[cfg(target_os = "macos")]
    register_application();

    let (artist, song) = util::split_artist_title(title);

    if let Err(err) = Notification::new()
        .appname(APP_DISPLAY_NAME)
        .summary(song)
        .body(artist.unwrap_or_default())
        // Icon-theme name on Linux (matches the installed hicolor icon);
        // ignored by the macOS and Windows backends.
        .icon(APP_ID)
        .show()
    {
        log::warn!("could not show notification: {err}");
    }
}

/// Register our bundle identifier with the macOS notification backend, once.
///
/// Otherwise `mac-notification-sys` lazily resolves a sending application on the
/// first notification by running the AppleScript `get id of application
/// "use_default"`. There is no app named `use_default`, so macOS pops a "Choose
/// Application" dialog. Setting our identifier up front consumes that one-time
/// initialisation and skips the lookup. Best-effort: it returns an error when
/// Whirr is not a registered app (e.g. under `cargo run`), which is harmless —
/// the dialog is suppressed either way.
#[cfg(target_os = "macos")]
fn register_application() {
    use std::sync::Once;

    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| {
        if let Err(err) = notify_rust::set_application(APP_ID) {
            log::debug!("could not register notification application: {err}");
        }
    });
}
