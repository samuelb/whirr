# 📻 Whirr

A tiny, native **system-tray player** for internet radio (MP3) streams. Point it
at any http(s) MP3 stream, then one click to play or pause, hover to see the
current track, with proper OS media-control and media-key integration and
automatic reconnect — written in Rust, packaged for every major desktop.

---

## Features

- ▶️ / ⏸️ **One-button play / pause** from the tray icon or its menu.
- 🔗 **Bring your own stream** — plays any http(s) MP3 stream you configure.
- 🎵 **Now-playing track** shown on hover (tray tooltip) and in the menu, parsed
  live from the stream's ICY metadata.
- 🎛️ **System media controls** — MPRIS on Linux, System Media Transport Controls
  on Windows, and the Now Playing center on macOS.
- ⌨️ **Media-key support** (Play/Pause/Stop keys) on all three platforms.
- 🔁 **Automatic reconnect** with exponential backoff when the stream drops.
- 🚀 **Start on login** — opt-in toggle right in the tray menu.
- 🪶 **Lightweight & self-contained** — a pure-Rust audio pipeline (no GStreamer,
  no VLC), so installation is a single small binary.
- 🖥️ **Cross-platform** — Linux (GNOME, KDE, and other StatusNotifier trays),
  macOS, and Windows.

## Supported platforms

| OS      | Architectures            | Packages                                    |
| ------- | ------------------------ | ------------------------------------------- |
| Linux   | `x86_64`, `aarch64`      | `.deb`, `.rpm`, Arch `PKGBUILD`, Nix flake  |
| macOS   | Apple Silicon + Intel    | universal `.dmg` (`.app` bundle)            |
| Windows | `x86_64`                 | NSIS installer `.exe` + portable `.exe`     |

---

## Installation

> Download the assets for your platform from the
> [latest release](https://github.com/samuelb/whirr/releases/latest).

### Debian / Ubuntu (`.deb`)

```bash
sudo apt install ./whirr_*_amd64.deb      # or _arm64.deb
```

Runtime dependencies (`libgtk-3`, `libayatana-appindicator3`, `libasound2`) are
pulled in automatically.

### Fedora / RHEL / openSUSE (`.rpm`)

```bash
sudo dnf install ./whirr-*.x86_64.rpm     # or .aarch64.rpm
```

### Arch Linux

```bash
# download PKGBUILD from the latest release assets, then:
makepkg -si
```

(An AUR package `whirr` can be published from the same `PKGBUILD`.)

### Nix / NixOS

```bash
# run without installing
nix run github:samuelb/whirr

# or add to a flake and install packages.default
nix profile install github:samuelb/whirr
```

### macOS

1. Download `whirr-macos.dmg`, open it, and drag **Whirr** to *Applications*.
2. On first launch, right-click the app and choose **Open** (the build is
   ad-hoc signed; a Developer ID signature can be added in CI).

The app runs as a menu-bar item with no Dock icon.

### Windows

- **Installer:** run `whirr-setup-<version>.exe` (adds a Start-menu
  shortcut and an uninstaller).
- **Portable:** just run `whirr-windows-x64.exe`.

### From source (any platform)

```bash
cargo install --path .
# or
cargo build --release   # binary at target/release/whirr
```

**Linux build dependencies:**

```bash
sudo apt-get install libgtk-3-dev libayatana-appindicator3-dev libasound2-dev pkg-config
```

---

## Usage

Launch **Whirr** (from your app launcher or the command line). A tray icon
appears.

**First run:** no stream is configured yet, so the tray shows *No stream URL
configured*. Choose **Set stream URL…** from the tray menu, enter the URL of
the MP3 stream you want to play, and playback starts right away. On later
launches Whirr starts playing automatically by default.

- **Left-click** the icon (Windows/macOS) to toggle play/pause.
- **Right-click** (or left-click on Linux) opens the menu:
  - the current track (or *Not playing*),
  - **Play / Pause**,
  - **Set stream URL…** (opens the configuration dialog),
  - **Start on login** (toggle),
  - **Autoplay on startup** (toggle),
  - **About**,
  - **Quit**.
- **Media keys** and your desktop's media widget / lock-screen controls work too.

### Diagnostics

```bash
whirr --selftest      # connect, decode silently for ~10s, print status
RUST_LOG=debug whirr  # verbose logging
```

---

## Configuration

The stream URL is set from the tray menu (**Set stream URL…**) and takes
effect immediately — no restart needed. The dialog is native on every
platform: a GTK dialog on Linux, an AppleScript dialog on macOS, and an input
box on Windows.

All settings (including the URL) are also stored as TOML; the file is created
on first launch and watched while the app runs — edits are applied
automatically within a couple of seconds, no restart needed. Location:

| OS      | Path                                                              |
| ------- | ---------------------------------------------------------------- |
| Linux   | `~/.config/whirr/config.toml`                              |
| macOS   | `~/Library/Application Support/io.github.samuelb.whirr/config.toml` |
| Windows | `%APPDATA%\samuelb\whirr\config\config.toml`                    |

```toml
# The http(s) MP3 stream to play. There is no default; set it here or via
# the "Set stream URL…" tray menu item.
stream_url = "https://example.com/stream.mp3"
# Output volume, 0.0–1.0
volume = 1.0
# Start playing automatically on launch
autoplay = true
# Kept in sync with the OS "start on login" state
autostart = false
```

---

## How it works

```
 HTTP (reqwest) ──► ICY de-mux ──► bounded channel ──► Symphonia decode ──► Rodio/CPAL ──► speakers
     │                  │                                   
     │                  └─► StreamTitle ──► tray tooltip + MPRIS/SMTC metadata
     └─► auto-reconnect with backoff
```

- **`reqwest`** streams the MP3 feed with `Icy-MetaData: 1`.
- **`icy`** splits interleaved metadata from audio and reports the `StreamTitle`.
- **`symphonia`** decodes MP3 frames; **`rodio`**/**`cpal`** play them.
- **`souvlaki`** provides MPRIS / SMTC / MediaRemote and media keys.
- **`tray-icon`** + **`tao`** provide the tray and event loop; **`auto-launch`**
  handles start-on-login.

Playback state uses a generation counter so pausing is instant and a single
worker ever feeds the audio device; the worker owns the reconnect loop, so the
stream self-heals without UI involvement. See `src/player.rs` for details.

---

## Development

```bash
cargo test            # unit tests (offline)
cargo fmt --all       # format
cargo clippy --all-targets
cargo run             # run locally
```

### CI / Releases

- **CI** (`.github/workflows/ci.yml`): fmt, Clippy (warnings-as-errors) and tests
  on Linux, macOS and Windows for every push/PR.
- **Release** (`.github/workflows/release.yml`): pushing a `vX.Y.Z` tag builds and
  publishes `.deb` + `.rpm` (amd64 & arm64), a universal macOS `.dmg`, and a
  Windows installer + portable exe, each with `SHA256SUMS`.

```bash
git tag v0.1.0 && git push origin v0.1.0    # cut a release
```

---

## License

Licensed under the MIT license ([LICENSE-MIT](LICENSE-MIT)).

Unless you explicitly state otherwise, any contribution you intentionally submit
for inclusion shall be licensed as above, without any additional terms or
conditions.
