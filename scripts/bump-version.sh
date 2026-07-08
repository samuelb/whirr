#!/usr/bin/env sh
# Bump the project to a release version: Cargo.toml, Cargo.lock and CHANGELOG.md.
# Usage: bump-version.sh <version> [repo-slug] [date]
#   version   release version without a leading "v" (e.g. 1.2.3)
#   repo-slug GitHub "owner/name" for CHANGELOG links (default: samuelb/gibbon)
#   date      release date YYYY-MM-DD (default: today, UTC)
set -eu

version="${1:?version required (X.Y.Z, no leading v)}"
repo="${2:-samuelb/gibbon}"
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

# --- Cargo.lock: bump the gibbon package entry so --locked builds still pass ---
awk -v ver="$version" '
  $0 == "name = \"gibbon\"" { in_pkg = 1 }
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

echo "bumped to $version ($date)"
