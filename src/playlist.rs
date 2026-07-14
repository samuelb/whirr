//! PLS playlist support: detecting a `.pls` response and extracting its
//! stream entries, so a playlist URL can stand in for a direct stream URL.
//!
//! PLS is the INI-style SHOUTcast/Icecast playlist format:
//!
//! ```text
//! [playlist]
//! File1=https://example.com/stream.mp3
//! Title1=Example Radio
//! Length1=-1
//! NumberOfEntries=1
//! Version=2
//! ```
//!
//! Only the `FileN` keys matter to us. Pure and unit-tested; the network side
//! (fetching the playlist and connecting to an entry) lives in `player`.

/// Whether a response looks like a PLS playlist rather than an audio stream,
/// judged by the URL path and/or the `Content-Type` header.
pub fn looks_like_pls(url_path: &str, content_type: Option<&str>) -> bool {
    if url_path.to_ascii_lowercase().ends_with(".pls") {
        return true;
    }
    let Some(content_type) = content_type else {
        return false;
    };
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    matches!(mime.as_str(), "audio/x-scpls" | "audio/scpls")
}

/// Extract the `FileN` entries from PLS text, ordered by their index.
/// Unknown keys and malformed lines are ignored, matching the lenient way
/// players have always treated the format.
pub fn parse_pls(text: &str) -> Vec<String> {
    let mut entries: Vec<(u32, String)> = Vec::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.len() <= 4 || !key[..4].eq_ignore_ascii_case("file") || value.is_empty() {
            continue;
        }
        let Ok(index) = key[4..].parse::<u32>() else {
            continue;
        };
        entries.push((index, value.to_string()));
    }
    entries.sort_by_key(|(index, _)| *index);
    entries.into_iter().map(|(_, url)| url).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_pls_by_extension() {
        assert!(looks_like_pls("/listen.pls", None));
        assert!(looks_like_pls("/LISTEN.PLS", None));
        assert!(!looks_like_pls("/stream.mp3", None));
    }

    #[test]
    fn detects_pls_by_content_type() {
        assert!(looks_like_pls("/listen", Some("audio/x-scpls")));
        assert!(looks_like_pls(
            "/listen",
            Some("audio/scpls; charset=utf-8")
        ));
        assert!(looks_like_pls("/listen", Some("Audio/X-SCPLS")));
        assert!(!looks_like_pls("/listen", Some("audio/mpeg")));
        assert!(!looks_like_pls("/listen", None));
    }

    #[test]
    fn parses_entries_in_index_order() {
        let text = "\
[playlist]
File2=https://example.com/two.mp3
Title2=Two
File1=https://example.com/one.mp3
Length1=-1
NumberOfEntries=2
Version=2
";
        assert_eq!(
            parse_pls(text),
            vec![
                "https://example.com/one.mp3".to_string(),
                "https://example.com/two.mp3".to_string(),
            ]
        );
    }

    #[test]
    fn ignores_malformed_and_unrelated_lines() {
        let text = "\
[playlist]
garbage
File=missing-index
Filex=bad-index
File1=
file2 = https://example.com/lenient.mp3
NumberOfEntries=1
";
        assert_eq!(
            parse_pls(text),
            vec!["https://example.com/lenient.mp3".to_string()]
        );
    }

    #[test]
    fn empty_playlist_yields_no_entries() {
        assert!(parse_pls("[playlist]\nNumberOfEntries=0\n").is_empty());
    }
}
