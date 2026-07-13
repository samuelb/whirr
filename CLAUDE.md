# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Whirr is a tiny, cross-platform (Linux/macOS/Windows) system-tray player for
internet radio (MP3) streams, written in Rust. It has no windowed UI — just a tray
icon, its menu, OS media-control integration, and one native stream-URL dialog.
The stream to play is set via the "Set stream URL…" menu item (persisted as
`stream_url` in the config file); there is deliberately **no built-in default
station** — do not add one. Without a URL the app runs with Play disabled until
the user sets one.

## Commands

```bash
cargo run                     # run locally (tray app)
cargo run -- --selftest       # headless: connect, decode silently ~10s, exit 0/1 (needs network, audio device + configured stream_url)
cargo test                    # unit tests, all offline
cargo test <name>             # single test, e.g. `cargo test clamps_out_of_range_volume`
cargo fmt --all               # format
cargo clippy --all-targets --all-features   # CI treats warnings as errors (RUSTFLAGS=-D warnings)
sh scripts/check-version-metadata.sh        # verify version is in sync across all packaging files
scripts/bump-version.sh <X.Y.Z>             # bump version everywhere at once (use this, don't hand-edit)
```

CI (`.github/workflows/ci.yml`) runs the version-metadata check, fmt, clippy, and tests
on all three OSes for every push/PR. Releases are cut by manually dispatching the
Release workflow; the next version is derived from conventional commits since the
last tag via git-cliff (`--bump`), overridable with the `bump` input
(auto/patch/minor/major).

## Version bumping (easy to get wrong)

The version lives in **many files** and CI fails if they drift: `Cargo.toml`, `flake.nix`,
`packaging/windows/installer.nsi`, both `CFBundleVersion` and
`CFBundleShortVersionString` in `packaging/macos/Info.plist`, and the Homebrew
templates `packaging/homebrew/Casks/whirr.rb` and `packaging/homebrew/Formula/whirr.rb`.
Always use `scripts/bump-version.sh` and confirm with `check-version-metadata.sh` — never
edit `Cargo.toml`'s version alone. (`packaging/arch/PKGBUILD` is deliberately
versionless: it is a generic VCS build whose version comes from `git describe`;
the release pipeline renders a pinned copy per release.)

## Branching

Trunk-based development: all commits always go to `main`. There are no long-lived
feature branches — commit directly to `main` (releases are cut from it by the
manually dispatched Release workflow, which derives the next `vX.Y.Z` tag from
the conventional commits and creates it).

Write commit subjects as Conventional Commits — `feat:`, `fix:`, `perf:`,
`refactor:`, `docs:`, `test:`, `chore:`, `ci:`, `build:` (with `!` for breaking
changes). Release notes are generated from them by git-cliff (`cliff.toml`);
unprefixed commits still appear, but only under a generic "Other" heading.

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

**Stream-URL dialog (`src/dialog.rs`).** One native prompt per platform, chosen to
avoid pulling in a GUI toolkit: GTK3 on Linux (already linked via tao/tray-icon),
`osascript` on macOS, PowerShell's `InputBox` on Windows. All three are
asynchronous — the result is delivered via a callback that posts
`UserEvent::StreamUrlEntered` back into the event loop (the callback may run on a
worker thread on macOS/Windows). `App` validates the input, saves the config, and
hands the new URL to the running engine via `Player::set_stream_url`, which
restarts the worker if playing — URL changes need no app restart.

**ICY demux (`src/icy.rs`).** A `Read` adapter that strips interleaved SHOUTcast/Icecast
metadata blocks (every `icy-metaint` bytes) from the audio and reports `StreamTitle` changes
through a callback. Pure and unit-tested.

**Config (`src/config.rs`).** TOML persisted in the platform config dir. `stream_url`
is an `Option<String>` with no default; `normalized()` clamps volume and turns
non-http(s)/unparsable stream URLs into `None` (`is_valid_stream_url` is the shared
check, also used by the dialog flow). Loading always falls back to defaults on any
error. On first launch a config file with a commented stream-URL hint is written
(`write_if_missing`). The URL set via the dialog applies immediately, and the file
itself is hot-reloaded: a `config-watcher` thread polls its contents (~2s) and the
app diffs the reloaded config against the running one (`App::on_config_file_changed`),
applying URL/volume/autostart changes live — invalid TOML during a reload is ignored
rather than reset to defaults (`load_strict` vs `load`). Also holds the app-wide constants
(`APP_DISPLAY_NAME`, `APP_ID`, `USER_AGENT`, etc.) — reuse these rather than
hardcoding strings.

## Conventions

- Errors use `anyhow` with `.context(...)`; the engine logs and recovers rather than
  crashing (playback errors surface as `PlaybackStatus::Error` and trigger reconnect).
- Platform-specific code is gated with `#[cfg(target_os = "...")]` inline; keep the three
  platforms working — CI builds all of them.
- Prefer the pure-Rust backends already chosen in `Cargo.toml` (rustls, `use_zbus`, notify's
  `z` feature) to avoid pulling in C build dependencies like libdbus/GStreamer.
