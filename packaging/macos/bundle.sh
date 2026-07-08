#!/usr/bin/env bash
#
# Assemble "Gibbon.app" from a compiled binary and (optionally) build a
# distributable .dmg.
#
# Usage:
#   packaging/macos/bundle.sh <path-to-gibbon-binary> [output-dir]
#
# Example (universal build):
#   cargo build --release --target aarch64-apple-darwin
#   cargo build --release --target x86_64-apple-darwin
#   lipo -create -output target/gibbon-universal \
#       target/aarch64-apple-darwin/release/gibbon \
#       target/x86_64-apple-darwin/release/gibbon
#   packaging/macos/bundle.sh target/gibbon-universal dist
#
set -euo pipefail

BIN="${1:?path to gibbon binary required}"
OUT_DIR="${2:-dist}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

APP="${OUT_DIR}/Gibbon.app"
CONTENTS="${APP}/Contents"

rm -rf "${APP}"
mkdir -p "${CONTENTS}/MacOS" "${CONTENTS}/Resources"

install -m 0755 "${BIN}" "${CONTENTS}/MacOS/gibbon"
cp "${ROOT}/packaging/macos/Info.plist" "${CONTENTS}/Info.plist"
cp "${ROOT}/assets/icons/gibbon.icns" "${CONTENTS}/Resources/gibbon.icns"

echo "Built ${APP}"

# Optional ad-hoc code signature (replace with your Developer ID for distribution).
if command -v codesign >/dev/null 2>&1; then
    codesign --force --deep --sign - "${APP}" || echo "warning: ad-hoc codesign failed"
fi

# Build a .dmg if hdiutil is available.
if command -v hdiutil >/dev/null 2>&1; then
    DMG="${OUT_DIR}/gibbon-macos.dmg"
    rm -f "${DMG}"
    STAGE="$(mktemp -d)"
    cp -R "${APP}" "${STAGE}/"
    ln -s /Applications "${STAGE}/Applications"
    hdiutil create -volname "Gibbon" -srcfolder "${STAGE}" -ov -format UDZO "${DMG}"
    rm -rf "${STAGE}"
    echo "Built ${DMG}"
fi
