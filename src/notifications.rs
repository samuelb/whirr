//! Desktop "now playing" notifications, shown when the track changes.

use notify_rust::Notification;

use crate::config::{APP_DISPLAY_NAME, APP_ID, STATION_NAME};
use crate::util;

/// Show a desktop notification announcing the newly playing track.
///
/// Best-effort: any failure (e.g. no notification daemon running) is logged and
/// otherwise ignored — a missing notification must never disrupt playback.
pub fn song_changed(title: &str) {
    let (artist, song) = util::split_artist_title(title);

    if let Err(err) = Notification::new()
        .appname(APP_DISPLAY_NAME)
        .summary(song)
        .body(artist.unwrap_or(STATION_NAME))
        // Icon-theme name on Linux (matches the installed hicolor icon);
        // ignored by the macOS and Windows backends.
        .icon(APP_ID)
        .show()
    {
        log::warn!("could not show notification: {err}");
    }
}
