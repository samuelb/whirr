# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- "Autoplay on startup" toggle in the tray menu (previously only settable via
  `autoplay` in the config file).

### Changed

- Gibbon is now a general-purpose stream player: there is no built-in default
  station anymore. The MP3 stream to play is set via the new **Set stream
  URL…** tray menu item, which opens a native dialog and applies the new URL
  immediately (it is persisted to `stream_url` in the config file). On a fresh
  install the tray shows "No stream URL configured" until a URL is set.
- The "Open … website" tray menu item was removed in favour of
  **Set stream URL…**.

## [0.3.0] - 2026-07-09

## [0.2.0] - 2026-07-08

## [0.1.1] - 2026-07-08

## [0.1.0] - 2026-07-08

### Added

- System-tray player for an internet radio (MP3) stream.
- One-button play/pause from the tray icon and menu.
- Now-playing track from ICY `StreamTitle`, shown in the tooltip and menu.
- System media controls and media-key support via MPRIS (Linux), SMTC
  (Windows) and MediaRemote (macOS).
- Automatic reconnect with exponential backoff.
- Opt-in "start on login".
- Pure-Rust audio pipeline (reqwest + symphonia + rodio), no native media stack.
- Packaging: `.deb`, `.rpm`, Arch `PKGBUILD`, macOS `.dmg`, Windows NSIS
  installer, and a Nix flake.
- GitHub Actions CI (fmt/clippy/test on Linux, macOS, Windows) and multi-platform
  release pipeline.

[Unreleased]: https://github.com/samuelb/gibbon/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/samuelb/gibbon/releases/tag/v0.3.0
[0.2.0]: https://github.com/samuelb/gibbon/releases/tag/v0.2.0
[0.1.1]: https://github.com/samuelb/gibbon/releases/tag/v0.1.1
[0.1.0]: https://github.com/samuelb/gibbon/releases/tag/v0.1.0
