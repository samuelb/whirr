//! Small cross-platform helpers.

/// Split an ICY `StreamTitle` of the form `Artist - Title` into its parts.
///
/// Returns `(artist, title)`. If there is no ` - ` separator the whole string
/// is treated as the title and the artist is `None`.
pub fn split_artist_title(s: &str) -> (Option<&str>, &str) {
    match s.split_once(" - ") {
        Some((artist, title)) if !artist.trim().is_empty() && !title.trim().is_empty() => {
            (Some(artist.trim()), title.trim())
        }
        _ => (None, s.trim()),
    }
}

/// Open a URL in the user's default browser. Best-effort; errors are logged.
pub fn open_url(url: &str) {
    let result = {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open").arg(url).spawn()
        }
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", url])
                .spawn()
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            std::process::Command::new("xdg-open").arg(url).spawn()
        }
    };
    if let Err(err) = result {
        log::warn!("could not open {url}: {err}");
    }
}
