//! Application constants and the persisted user configuration.
//!
//! The stream and station referenced here belong to Example Radio
//! (<https://example.com/>). This project is an unofficial, independent client
//! and is not affiliated with, endorsed by, or connected to example.com.

use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

/// The Example Radio stream endpoint (upstream). See <https://example.com/>.
pub const STREAM_URL: &str = "https://example.com/stream.mp3";
/// Human-readable station name, shown in menus, tooltips and media controls.
pub const STATION_NAME: &str = "Example Radio";
/// The station's website. Opened from the tray menu.
pub const STATION_URL: &str = "https://example.com/";
/// This project's repository (shown in the About entry).
pub const REPO_URL: &str = "https://github.com/samuelb/gibbon";
/// The application's own name (this client), distinct from the station name.
/// Shown to the OS for media controls / the login item.
pub const APP_DISPLAY_NAME: &str = "Gibbon";
/// Reverse-DNS application identifier (bundle id / desktop file base name).
/// Used by packaging (bundle id, icon/desktop file names) rather than at runtime.
#[allow(dead_code)]
pub const APP_ID: &str = "io.github.samuelb.gibbon";
/// D-Bus well-known name element used for the MPRIS interface on Linux.
pub const DBUS_NAME: &str = "gibbon";
/// User-Agent sent when connecting to the stream.
pub const USER_AGENT: &str = concat!(
    "gibbon/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/samuelb/gibbon)"
);

/// Persisted user settings, stored as TOML in the platform config directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Stream URL to play. Overridable for testing or alternate mounts.
    pub stream_url: String,
    /// Output volume in the range `0.0..=1.0`.
    pub volume: f32,
    /// Start playing automatically when the app launches.
    pub autoplay: bool,
    /// Whether the app is registered to start on login. Kept in sync with the OS.
    pub autostart: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            stream_url: STREAM_URL.to_string(),
            volume: 1.0,
            autoplay: true,
            autostart: false,
        }
    }
}

impl Config {
    /// Path to the on-disk config file, if a config directory can be determined.
    pub fn path() -> Option<PathBuf> {
        ProjectDirs::from("io.github", "samuelb", "gibbon")
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }

    /// Load config from disk, falling back to defaults on any error.
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str(&text) {
                Ok(cfg) => cfg,
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

    /// Persist the config to disk, creating the parent directory as needed.
    pub fn save(&self) -> Result<()> {
        let path = Self::path().context("no config directory available")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating config dir {}", parent.display()))?;
        }
        let text = toml::to_string_pretty(self).context("serializing config")?;
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }
}
