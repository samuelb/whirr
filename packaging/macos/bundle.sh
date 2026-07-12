#!/usr/bin/env bash
#
# Assemble "Whirr.app" from a compiled binary and (optionally) build a
# distributable .dmg.
#
# Usage:
#   packaging/macos/bundle.sh <path-to-whirr-binary> [output-dir]
#
# Example (universal build):
#   cargo build --release --target aarch64-apple-darwin
#   cargo build --release --target x86_64-apple-darwin
#   lipo -create -output target/whirr-universal \
#       target/aarch64-apple-darwin/release/whirr \
#       target/x86_64-apple-darwin/release/whirr
#   packaging/macos/bundle.sh target/whirr-universal dist
#
set -euo pipefail

BIN="${1:?path to whirr binary required}"
OUT_DIR="${2:-dist}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

APP="${OUT_DIR}/Whirr.app"
CONTENTS="${APP}/Contents"

rm -rf "${APP}"
mkdir -p "${CONTENTS}/MacOS" "${CONTENTS}/Resources"

install -m 0755 "${BIN}" "${CONTENTS}/MacOS/whirr"
cp "${ROOT}/packaging/macos/Info.plist" "${CONTENTS}/Info.plist"
cp "${ROOT}/assets/icons/whirr.icns" "${CONTENTS}/Resources/whirr.icns"

echo "Built ${APP}"

# Optional ad-hoc code signature (replace with your Developer ID for distribution).
if command -v codesign >/dev/null 2>&1; then
    codesign --force --deep --sign - "${APP}" || echo "warning: ad-hoc codesign failed"
fi

# Build a .dmg if hdiutil is available.
if command -v hdiutil >/dev/null 2>&1; then
    DMG="${OUT_DIR}/whirr-macos.dmg"
    rm -f "${DMG}"
    STAGE="$(mktemp -d)"
    cp -R "${APP}" "${STAGE}/"
    ln -s /Applications "${STAGE}/Applications"
    hdiutil create -volname "Whirr" -srcfolder "${STAGE}" -ov -format UDZO "${DMG}"
    rm -rf "${STAGE}"
    echo "Built ${DMG}"
fi
