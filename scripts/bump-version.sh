#!/usr/bin/env sh
# Bump the project to a release version across every file the CI version check
# (scripts/check-version-metadata.sh) guards: Cargo.toml, Cargo.lock,
# flake.nix, the NSIS installer, the macOS Info.plist and the Homebrew
# cask/formula templates. Keep the patterns below in sync with that check.
# (The Arch PKGBUILD is deliberately not bumped: it is a generic VCS build
# whose version comes from `git describe` at build time.)
# Usage: bump-version.sh <version>
#   version   release version without a leading "v" (e.g. 1.2.3)
set -eu

version="${1:?version required (X.Y.Z, no leading v)}"
tmp="$(mktemp)"

# --- Cargo.toml: bump the version within the [package] section ---
awk -v ver="$version" '
  /^\[/ { section = $0 }
  section == "[package]" && /^version[[:space:]]*=/ && !done {
    sub(/"[^"]*"/, "\"" ver "\""); done = 1
  }
  { print }
' Cargo.toml > "$tmp" && mv "$tmp" Cargo.toml

# --- Cargo.lock: bump the whirr package entry so --locked builds still pass ---
awk -v ver="$version" '
  $0 == "name = \"whirr\"" { in_pkg = 1 }
  in_pkg && /^version[[:space:]]*=/ { sub(/"[^"]*"/, "\"" ver "\""); in_pkg = 0 }
  { print }
' Cargo.lock > "$tmp" && mv "$tmp" Cargo.lock

# --- flake.nix: bump the Nix package `version` attribute ---
awk -v ver="$version" '
  !done && /^[[:space:]]*version[[:space:]]*=[[:space:]]*"[^"]*";[[:space:]]*$/ {
    sub(/"[^"]*"/, "\"" ver "\""); done = 1
  }
  { print }
' flake.nix > "$tmp" && mv "$tmp" flake.nix

# --- NSIS installer: bump the VERSION define ---
awk -v ver="$version" '
  !done && /^[[:space:]]*!define VERSION "[^"]*"/ {
    sub(/"[^"]*"/, "\"" ver "\""); done = 1
  }
  { print }
' packaging/windows/installer.nsi > "$tmp" && mv "$tmp" packaging/windows/installer.nsi

# --- macOS Info.plist: bump the <string> following each version <key> ---
awk -v ver="$version" '
  bump { sub(/<string>[^<]*<\/string>/, "<string>" ver "</string>"); bump = 0 }
  /<key>CFBundleVersion<\/key>/ || /<key>CFBundleShortVersionString<\/key>/ { bump = 1 }
  { print }
' packaging/macos/Info.plist > "$tmp" && mv "$tmp" packaging/macos/Info.plist

# --- Homebrew cask and formula templates: bump the version stanza ---
for rb in packaging/homebrew/Casks/whirr.rb packaging/homebrew/Formula/whirr.rb; do
    awk -v ver="$version" '
      !done && /^[[:space:]]*version "[^"]*"/ {
        sub(/"[^"]*"/, "\"" ver "\""); done = 1
      }
      { print }
    ' "$rb" > "$tmp" && mv "$tmp" "$rb"
done

echo "bumped to $version"
