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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_artist_and_title() {
        assert_eq!(
            split_artist_title(" Artist - Track "),
            (Some("Artist"), "Track")
        );
    }

    #[test]
    fn treats_missing_or_empty_parts_as_title_only() {
        assert_eq!(split_artist_title("Track"), (None, "Track"));
        assert_eq!(split_artist_title("Artist - "), (None, "Artist -"));
        assert_eq!(split_artist_title(" - Track"), (None, "- Track"));
    }
}
