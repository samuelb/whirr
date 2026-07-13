#!/usr/bin/env sh
# Render the Homebrew tap files from the templates in packaging/homebrew/
# (a tree mirroring the samuelb/homebrew-tap layout: Casks/whirr.rb for the
# macOS .app, Formula/whirr.rb for the plain binary on macOS and Linux) by
# stamping in the release version and the published artifacts' checksums.
# The rendered tree is what the release workflow hands to the tap's
# reusable publish workflow. Mirrors somad's scripts/stage-homebrew.sh.
# Usage: stage-homebrew.sh <version> <checksums.txt> [out_dir]
#   version   release version without a leading "v" (e.g. 1.2.3)
set -eu

version="${1:?version required (X.Y.Z, no leading v)}"
checksums="${2:?path to checksums.txt required}"
out_dir="${3:-tap}"

sha() {
    grep " $1\$" "$checksums" | awk '{ print $1 }'
}

macos_dmg="$(sha whirr-macos.dmg)"
macos_universal="$(sha whirr-macos-universal.tar.gz)"
linux_arm64="$(sha whirr-linux-arm64.tar.gz)"
linux_amd64="$(sha whirr-linux-amd64.tar.gz)"

for name_value in "whirr-macos.dmg:$macos_dmg" "whirr-macos-universal.tar.gz:$macos_universal" "whirr-linux-arm64.tar.gz:$linux_arm64" "whirr-linux-amd64.tar.gz:$linux_amd64"; do
    if [ -z "${name_value#*:}" ]; then
        echo "missing checksum for ${name_value%%:*} in $checksums" >&2
        exit 1
    fi
done

mkdir -p "$out_dir"
cp -R packaging/homebrew/. "$out_dir/"

tmp="$(mktemp)"
for rb in "$out_dir/Casks/whirr.rb" "$out_dir/Formula/whirr.rb"; do
    sed \
        -e "s|^  version \".*\"|  version \"$version\"|" \
        -e "s|REPLACE_WITH_MACOS_DMG_SHA256|$macos_dmg|" \
        -e "s|REPLACE_WITH_MACOS_UNIVERSAL_SHA256|$macos_universal|" \
        -e "s|REPLACE_WITH_LINUX_ARM64_SHA256|$linux_arm64|" \
        -e "s|REPLACE_WITH_LINUX_AMD64_SHA256|$linux_amd64|" \
        "$rb" > "$tmp" && mv "$tmp" "$rb"
done

if grep -R "REPLACE_WITH" "$out_dir" >/dev/null 2>&1; then
    echo "unstamped placeholders remain in $out_dir:" >&2
    grep -Rn "REPLACE_WITH" "$out_dir" >&2
    exit 1
fi

echo "staged Homebrew tap files in $out_dir (version $version)"
