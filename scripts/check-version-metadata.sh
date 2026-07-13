#!/usr/bin/env sh
set -eu

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"

check() {
    name="$1"
    value="$2"
    if [ "$value" != "$version" ]; then
        echo "$name version is $value, expected $version" >&2
        exit 1
    fi
}

check "flake.nix" "$(sed -n 's/^[[:space:]]*version = "\(.*\)";/\1/p' flake.nix | head -n 1)"
check "NSIS" "$(sed -n 's/^[[:space:]]*!define VERSION "\(.*\)"/\1/p' packaging/windows/installer.nsi | head -n 1)"
check "Info.plist CFBundleVersion" "$(
    awk '
        /<key>CFBundleVersion<\/key>/ { getline; gsub(/^[[:space:]]*<string>|<\/string>[[:space:]]*$/, ""); print; exit }
    ' packaging/macos/Info.plist
)"
check "Info.plist CFBundleShortVersionString" "$(
    awk '
        /<key>CFBundleShortVersionString<\/key>/ { getline; gsub(/^[[:space:]]*<string>|<\/string>[[:space:]]*$/, ""); print; exit }
    ' packaging/macos/Info.plist
)"
check "Homebrew cask" "$(sed -n 's/^[[:space:]]*version "\(.*\)"/\1/p' packaging/homebrew/Casks/whirr.rb | head -n 1)"
check "Homebrew formula" "$(sed -n 's/^[[:space:]]*version "\(.*\)"/\1/p' packaging/homebrew/Formula/whirr.rb | head -n 1)"

echo "version metadata matches $version"
