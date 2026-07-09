# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Gibbon is a tiny, cross-platform (Linux/macOS/Windows) system-tray player for the
Example Radio internet stream, written in Rust. It has no windowed UI — just a tray
icon, its menu, and OS media-control integration. It is an **unofficial** client with
no affiliation to example.com; keep that framing intact in user-facing strings and docs.

## Commands

```bash
cargo run                     # run locally (tray app)
cargo run -- --selftest       # headless: connect, decode silently ~10s, exit 0/1 (needs network + audio device)
cargo test                    # unit tests, all offline
cargo test <name>             # single test, e.g. `cargo test clamps_out_of_range_volume`
cargo fmt --all               # format
cargo clippy --all-targets --all-features   # CI treats warnings as errors (RUSTFLAGS=-D warnings)
sh scripts/check-version-metadata.sh        # verify version is in sync across all packaging files
scripts/bump-version.sh <X.Y.Z>             # bump version everywhere at once (use this, don't hand-edit)
```

CI (`.github/workflows/ci.yml`) runs the version-metadata check, fmt, clippy, and tests
on all three OSes for every push/PR. Releases are cut by pushing a `vX.Y.Z` tag.

## Version bumping (easy to get wrong)

The version lives in **many files** and CI fails if they drift: `Cargo.toml`, `flake.nix`,
`packaging/arch/PKGBUILD`, `packaging/windows/installer.nsi`, and both `CFBundleVersion`
and `CFBundleShortVersionString` in `packaging/macos/Info.plist`. Always use
`scripts/bump-version.sh` and confirm with `check-version-metadata.sh` — never edit
`Cargo.toml`'s version alone.

## Branching

Trunk-based development: all commits always go to `main`. There are no long-lived
feature branches — commit directly to `main` (releases are cut by tagging `vX.Y.Z` on it).

## Architecture

Two long-lived subsystems connected by message passing, plus platform integration modules.

**UI / event loop (`src/app.rs`, `src/main.rs`).** `tao`'s event loop is the single UI
thread. All inputs — tray clicks, menu selections, OS media-control events, and `Player`
events — are funnelled into one `UserEvent` enum via an `EventLoopProxy` and handled by the
`App` struct. `App` holds the source-of-truth UI state (status, title, `last_song`) and
pushes it out to the tray icon/menu (`src/tray.rs`) and to `souvlaki` media controls
(`src/controls.rs`). A hidden window is created solely because the Windows SMTC backend
requires a window handle. On macOS the app runs as an `Accessory` (no Dock icon).

**Audio engine (`src/player.rs`).** Runs entirely off the UI thread and reports back via an
`emit` callback (which posts `PlayerEvent`s into the event loop). Its concurrency model is
the crux of the codebase:
- One **engine thread** owns the non-`Send` audio output device and processes `Command`s.
- Playback is a **monotonic generation counter** (`AtomicUsize`). Play bumps it and spawns a
  **worker** tagged with that generation; pause/stop just bumps it again. A worker runs only
  while its generation is current (`should_run()`), so pause is instant and there's never
  more than one worker feeding the device.
- The worker owns the **reconnect loop** (exponential backoff, capped at 30s), so the stream
  self-heals without any UI involvement.
- Within a session a **network thread** reads HTTP + demuxes ICY metadata, forwarding audio
  bytes over a bounded `crossbeam` channel (natural back-pressure). The worker decodes from
  the channel via `ChannelSource` (a `Send + Sync` `MediaSource`, which the raw HTTP response
  is not). Pipeline: reqwest → `IcyReader` demux → channel → Symphonia decode → Rodio/CPAL.

**ICY demux (`src/icy.rs`).** A `Read` adapter that strips interleaved SHOUTcast/Icecast
metadata blocks (every `icy-metaint` bytes) from the audio and reports `StreamTitle` changes
through a callback. Pure and unit-tested.

**Config (`src/config.rs`).** TOML persisted in the platform config dir. Loading always
falls back to defaults on any error and runs `normalized()` (clamps volume, rejects
non-http(s) stream URLs). Also holds the app-wide constants (`STATION_NAME`, `APP_ID`,
`USER_AGENT`, etc.) — reuse these rather than hardcoding strings.

## Conventions

- Errors use `anyhow` with `.context(...)`; the engine logs and recovers rather than
  crashing (playback errors surface as `PlaybackStatus::Error` and trigger reconnect).
- Platform-specific code is gated with `#[cfg(target_os = "...")]` inline; keep the three
  platforms working — CI builds all of them.
- Prefer the pure-Rust backends already chosen in `Cargo.toml` (rustls, `use_zbus`, notify's
  `z` feature) to avoid pulling in C build dependencies like libdbus/GStreamer.
