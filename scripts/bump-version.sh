#!/usr/bin/env sh
# Bump the project to a release version across every file the CI version check
# (scripts/check-version-metadata.sh) guards: Cargo.toml, Cargo.lock,
# CHANGELOG.md, flake.nix, the Arch PKGBUILD, the NSIS installer and the macOS
# Info.plist. Keep the patterns below in sync with that check.
# Usage: bump-version.sh <version> [repo-slug] [date]
#   version   release version without a leading "v" (e.g. 1.2.3)
#   repo-slug GitHub "owner/name" for CHANGELOG links (default: samuelb/whirr)
#   date      release date YYYY-MM-DD (default: today, UTC)
set -eu

version="${1:?version required (X.Y.Z, no leading v)}"
repo="${2:-samuelb/whirr}"
date="${3:-$(date -u +%F)}"
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

# --- CHANGELOG.md: promote [Unreleased] to the new version and refresh links ---
awk -v ver="$version" -v date="$date" -v repo="$repo" '
  /^## \[Unreleased\]/ && !header {
    print; print ""; print "## [" ver "] - " date
    header = 1; next
  }
  /^\[Unreleased\]:/ && !link {
    print "[Unreleased]: https://github.com/" repo "/compare/v" ver "...HEAD"
    print "[" ver "]: https://github.com/" repo "/releases/tag/v" ver
    link = 1; next
  }
  { print }
' CHANGELOG.md > "$tmp" && mv "$tmp" CHANGELOG.md

# --- flake.nix: bump the Nix package `version` attribute ---
awk -v ver="$version" '
  !done && /^[[:space:]]*version[[:space:]]*=[[:space:]]*"[^"]*";[[:space:]]*$/ {
    sub(/"[^"]*"/, "\"" ver "\""); done = 1
  }
  { print }
' flake.nix > "$tmp" && mv "$tmp" flake.nix

# --- Arch PKGBUILD: bump pkgver and reset pkgrel for the new upstream version ---
awk -v ver="$version" '
  /^pkgver=/ { print "pkgver=" ver; next }
  /^pkgrel=/ { print "pkgrel=1"; next }
  { print }
' packaging/arch/PKGBUILD > "$tmp" && mv "$tmp" packaging/arch/PKGBUILD

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

echo "bumped to $version ($date)"
