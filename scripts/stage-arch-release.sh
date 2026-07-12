#!/usr/bin/env sh
set -eu

version="${1:?version required}"
out_dir="${2:-dist}"
pkgname="whirr"
tarball="$pkgname-$version.tar.gz"

mkdir -p "$out_dir"
git archive --format=tar.gz --prefix="$pkgname-$version/" -o "$out_dir/$tarball" HEAD

if command -v sha256sum >/dev/null 2>&1; then
    checksum="$(sha256sum "$out_dir/$tarball" | awk '{ print $1 }')"
else
    checksum="$(shasum -a 256 "$out_dir/$tarball" | awk '{ print $1 }')"
fi

sed \
    -e "s|^source=.*|source=(\"\$pkgname-\$pkgver.tar.gz::https://github.com/samuelb/whirr/releases/download/v\$pkgver/\$pkgname-\$pkgver.tar.gz\")|" \
    -e "s|^sha256sums=.*|sha256sums=('$checksum')|" \
    packaging/arch/PKGBUILD > "$out_dir/PKGBUILD"

echo "staged $out_dir/$tarball and $out_dir/PKGBUILD"
