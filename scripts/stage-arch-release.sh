#!/usr/bin/env sh
# Stage the Arch release artifacts: a source tarball (git archive of HEAD)
# and a pinned PKGBUILD rendered from the generic VCS one committed at
# packaging/arch/PKGBUILD (fixed pkgver, release-tarball source, real
# checksum, no pkgver() function). Both are attached to the GitHub release.
# Mirrors somad's scripts/stage-arch-release.sh.
# Usage: stage-arch-release.sh <version> [out_dir]
#   version   release version without a leading "v" (e.g. 1.2.3)
set -eu

version="${1:?version required (X.Y.Z, no leading v)}"
out_dir="${2:-dist}"
pkgname="whirr"
tarball="$pkgname-$version.tar.gz"

mkdir -p "$out_dir"
# --prefix is unversioned so `cd "$pkgname"` works the same in the VCS and
# pinned PKGBUILD variants.
git archive --format=tar.gz --prefix="$pkgname/" -o "$out_dir/$tarball" HEAD

if command -v sha256sum >/dev/null 2>&1; then
    checksum="$(sha256sum "$out_dir/$tarball" | awk '{ print $1 }')"
else
    checksum="$(shasum -a 256 "$out_dir/$tarball" | awk '{ print $1 }')"
fi

awk -v ver="$version" -v sum="$checksum" -v q="'" '
  # Drop the pkgver() function: the release tarball carries no git metadata
  # to derive a version from, and pkgver is pinned below.
  /^pkgver\(\) \{/ { skip = 1; next }
  skip && /^\}$/ { skip = 0; eat_blank = 1; next }
  skip { next }
  eat_blank { eat_blank = 0; if ($0 == "") next }
  /^pkgver=/ { print "pkgver=" ver; next }
  /^makedepends=/ { sub(/ *'\''git'\''/, ""); print; next }
  /^source=/ {
    print "source=(\"$pkgname-$pkgver.tar.gz::$url/releases/download/v$pkgver/$pkgname-$pkgver.tar.gz\")"
    next
  }
  /^sha256sums=/ { print "sha256sums=(" q sum q ")"; next }
  { print }
' packaging/arch/PKGBUILD > "$out_dir/PKGBUILD"

echo "staged $out_dir/$tarball and $out_dir/PKGBUILD (version $version, sha256 $checksum)"
