#!/usr/bin/env sh
# Produce the published Homebrew cask from packaging/homebrew/whirr.rb by
# stamping in the release version and the checksum of the built .dmg. Mirrors
# scripts/stage-arch-release.sh. The result is what gets pushed to the
# samuelb/homebrew-tap by the release workflow.
# Usage: stage-homebrew-cask.sh <version> <path-to-whirr-macos.dmg> [output]
set -eu

version="${1:?version required (X.Y.Z, no leading v)}"
dmg="${2:?path to whirr-macos.dmg required}"
out="${3:-Casks/whirr.rb}"

if command -v sha256sum >/dev/null 2>&1; then
    checksum="$(sha256sum "$dmg" | awk '{ print $1 }')"
else
    checksum="$(shasum -a 256 "$dmg" | awk '{ print $1 }')"
fi

mkdir -p "$(dirname "$out")"
sed \
    -e "s|^  version \".*\"|  version \"$version\"|" \
    -e "s|^  sha256 \".*\"|  sha256 \"$checksum\"|" \
    packaging/homebrew/whirr.rb > "$out"

echo "staged $out (version $version, sha256 $checksum)"
