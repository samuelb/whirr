//! Application constants and the persisted user configuration.

use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

/// This project's repository (shown in the About entry).
pub const REPO_URL: &str = "https://github.com/samuelb/whirr";
/// The application's name, shown to the OS for media controls / the login item.
pub const APP_DISPLAY_NAME: &str = "Whirr";
/// Reverse-DNS application identifier (bundle id / desktop file base name).
/// Used by packaging (bundle id, icon/desktop file names) and as the desktop
/// notification icon name (matches the installed hicolor icon on Linux).
pub const APP_ID: &str = "io.github.samuelb.whirr";
/// D-Bus well-known name element used for the MPRIS interface on Linux.
pub const DBUS_NAME: &str = "whirr";
/// User-Agent sent when connecting to the stream.
pub const USER_AGENT: &str = concat!(
    "whirr/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/samuelb/whirr)"
);

/// Comment block written above the config when no stream URL is set yet,
/// telling the user what to add.
const STREAM_URL_HINT: &str = "\
# No stream URL is configured. Use \"Set stream URL…\" in the tray menu, or set
# stream_url here, e.g.:
#
#     stream_url = \"https://example.com/stream.mp3\"
#
# Changes to this file are applied automatically while the app is running.

";

/// Whether `s` parses as an http(s) URL — the only kind of stream we accept.
pub fn is_valid_stream_url(s: &str) -> bool {
    matches!(s.parse::<reqwest::Url>(), Ok(url) if matches!(url.scheme(), "http" | "https"))
}

/// Persisted user settings, stored as TOML in the platform config directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The http(s) MP3/AAC stream to play. There is no default: the user must set
    /// one in the config file before playback is possible.
    pub stream_url: Option<String>,
    /// Output volume in the range `0.0..=1.0`.
    pub volume: f32,
    /// Start playing automatically when the app launches.
    pub autoplay: bool,
    /// Whether the app is registered to start on login. Kept in sync with the OS.
    pub autostart: bool,
    /// Show a desktop notification when the playing track changes.
    pub notifications: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            stream_url: None,
            volume: 1.0,
            autoplay: false,
            autostart: false,
            notifications: true,
        }
    }
}

impl Config {
    /// Path to the on-disk config file, if a config directory can be determined.
    /// The directory is named plain `whirr` on every platform (e.g.
    /// `~/.config/whirr`, `~/Library/Application Support/whirr`).
    pub fn path() -> Option<PathBuf> {
        ProjectDirs::from("", "", "whirr").map(|dirs| dirs.config_dir().join("config.toml"))
    }

    /// The config file's location before 0.4, when the directory was named
    /// after the full bundle id (`io.github.samuelb.whirr` on macOS,
    /// `samuelb\whirr` on Windows). On Linux it was already plain `whirr`.
    fn legacy_path() -> Option<PathBuf> {
        ProjectDirs::from("io.github", "samuelb", "whirr")
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }

    /// Move the config file from [`Self::legacy_path`] to [`Self::path`], so
    /// existing settings survive the directory rename. Best-effort: on any
    /// failure the old file stays put and a fresh config is used instead.
    pub fn migrate_legacy_path() {
        let (Some(old), Some(new)) = (Self::legacy_path(), Self::path()) else {
            return;
        };
        if old == new || !old.exists() || new.exists() {
            return;
        }
        let moved = match new.parent() {
            Some(parent) => std::fs::create_dir_all(parent),
            None => Ok(()),
        }
        .and_then(|()| std::fs::rename(&old, &new));
        match moved {
            Ok(()) => {
                log::info!("moved config from {} to {}", old.display(), new.display());
                // Remove the legacy directory; fails harmlessly if not empty.
                if let Some(dir) = old.parent() {
                    let _ = std::fs::remove_dir(dir);
                }
            }
            Err(err) => log::warn!(
                "could not move config from {} to {}: {err}",
                old.display(),
                new.display()
            ),
        }
    }

    /// Load config from disk, falling back to defaults on any error.
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Self>(&text) {
                Ok(cfg) => cfg.normalized(),
                Err(err) => {
                    log::warn!(
                        "invalid config at {}: {err}; using defaults",
                        path.display()
                    );
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Load and normalize the config from disk, erroring on a missing or
    /// unparsable file. Used by the file watcher, which must ignore a
    /// half-written file instead of falling back to defaults.
    pub fn load_strict() -> Result<Self> {
        let path = Self::path().context("no config directory available")?;
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let cfg: Self = toml::from_str(&text).context("parsing config")?;
        Ok(cfg.normalized())
    }

    /// Persist the config to disk, creating the parent directory as needed.
    pub fn save(&self) -> Result<()> {
        let path = Self::path().context("no config directory available")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating config dir {}", parent.display()))?;
        }
        let mut text = toml::to_string_pretty(self).context("serializing config")?;
        if self.stream_url.is_none() {
            text.insert_str(0, STREAM_URL_HINT);
        }
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    /// Write the config file if it does not exist yet, so users have a file
    /// (with the stream-URL hint) to edit. Best-effort.
    pub fn write_if_missing(&self) {
        if let Some(path) = Self::path() {
            if !path.exists() {
                if let Err(err) = self.save() {
                    log::warn!("could not write initial config: {err:#}");
                }
            }
        }
    }

    /// Spawn a thread that polls the config file and calls `on_change`
    /// whenever its contents change (a couple of seconds of latency).
    /// Comparing contents — not mtimes — coalesces multi-step editor saves
    /// and survives atomic-rename writes, which break naive path watchers.
    pub fn spawn_watcher<F>(on_change: F)
    where
        F: Fn() + Send + 'static,
    {
        let Some(path) = Self::path() else {
            return;
        };
        let spawned = std::thread::Builder::new()
            .name("config-watcher".into())
            .spawn(move || {
                let mut last = std::fs::read(&path).ok();
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let current = std::fs::read(&path).ok();
                    if current != last {
                        last = current;
                        on_change();
                    }
                }
            });
        if let Err(err) = spawned {
            log::warn!("could not start config watcher: {err}");
        }
    }

    /// Return a config with out-of-range or unsupported values repaired.
    pub(crate) fn normalized(mut self) -> Self {
        if !self.volume.is_finite() {
            log::warn!("invalid volume {}; using default", self.volume);
            self.volume = Self::default().volume;
        } else {
            let clamped = self.volume.clamp(0.0, 1.0);
            if clamped != self.volume {
                log::warn!("volume {} outside 0.0..=1.0; clamping", self.volume);
                self.volume = clamped;
            }
        }

        if let Some(url) = &self.stream_url {
            if !is_valid_stream_url(url) {
                log::warn!("invalid or non-http(s) stream_url {url}; ignoring it");
                self.stream_url = None;
            }
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_no_stream_url() {
        assert_eq!(Config::default().stream_url, None);
    }

    #[test]
    fn normalizes_invalid_volume() {
        let cfg = Config {
            volume: f32::NAN,
            ..Config::default()
        }
        .normalized();

        assert_eq!(cfg.volume, Config::default().volume);
    }

    #[test]
    fn clamps_out_of_range_volume() {
        let high = Config {
            volume: 1.5,
            ..Config::default()
        }
        .normalized();
        let low = Config {
            volume: -0.5,
            ..Config::default()
        }
        .normalized();

        assert_eq!(high.volume, 1.0);
        assert_eq!(low.volume, 0.0);
    }

    #[test]
    fn rejects_invalid_stream_url() {
        let unsupported_scheme = Config {
            stream_url: Some("file:///tmp/audio.mp3".to_string()),
            ..Config::default()
        }
        .normalized();
        let unparsable = Config {
            stream_url: Some("not a url".to_string()),
            ..Config::default()
        }
        .normalized();

        assert_eq!(unsupported_scheme.stream_url, None);
        assert_eq!(unparsable.stream_url, None);
    }

    #[test]
    fn keeps_valid_stream_url() {
        let cfg = Config {
            stream_url: Some("https://example.com/stream.mp3".to_string()),
            ..Config::default()
        }
        .normalized();

        assert_eq!(
            cfg.stream_url.as_deref(),
            Some("https://example.com/stream.mp3")
        );
    }

    #[test]
    fn serializes_and_reloads_config_without_stream_url() {
        let text = toml::to_string_pretty(&Config::default()).unwrap();
        assert!(!text.contains("stream_url"));

        let reloaded: Config = toml::from_str(&text).unwrap();
        assert_eq!(reloaded.stream_url, None);
    }
}
