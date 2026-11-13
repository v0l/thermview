#!/usr/bin/env bash
# Build on the oldest Debian/Ubuntu release you intend to support.
# Usage: packaging/linux/build-deb.sh [native Rust target]
# THERMVIEW_BINARY can point to an already-built binary (also used by CI).
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo"
for tool in cargo rustc python3 dpkg-deb dpkg-shlibdeps dpkg-architecture strip; do
    command -v "$tool" >/dev/null || { printf 'Missing build tool: %s\n' "$tool" >&2; exit 1; }
done
host=$(rustc -vV | sed -n 's/^host: //p')
target=${1:-$host}
case "$target" in
    x86_64-unknown-linux-gnu) arch=amd64 ;;
    aarch64-unknown-linux-gnu) arch=arm64 ;;
    *) printf 'Unsupported Debian target: %s\n' "$target" >&2; exit 1 ;;
esac
if [ "$target" != "$host" ] || [ "$arch" != "$(dpkg-architecture -qDEB_BUILD_ARCH)" ]; then
    printf 'Build this package on a native %s Debian/Ubuntu runner.\n' "$arch" >&2
    exit 1
fi
version=$(cargo metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "thermview"))')
version=${version//-/~}-1
binary=${THERMVIEW_BINARY:-$repo/target/$target/release/thermview}
if [ -z "${THERMVIEW_BINARY:-}" ]; then
    cargo build --release --locked -p thermview --target "$target"
fi
binary=$(realpath "$binary")
out="$repo/dist"
mkdir -p "$out"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
root="$work/package"
install -Dm755 "$binary" "$root/usr/bin/thermview"
strip --strip-unneeded "$root/usr/bin/thermview"
install -Dm644 packaging/linux/thermview.desktop "$root/usr/share/applications/thermview.desktop"
install -Dm644 packaging/linux/thermview.xml "$root/usr/share/mime/packages/thermview.xml"
install -Dm644 web/public/favicon.svg "$root/usr/share/icons/hicolor/scalable/apps/thermview.svg"
install -Dm644 web/public/icon-192.png "$root/usr/share/icons/hicolor/192x192/apps/thermview.png"
install -Dm644 LICENSE "$root/usr/share/doc/thermview/copyright"
install -Dm644 README.md "$root/usr/share/doc/thermview/README.md"
gzip -9 -n "$root/usr/share/doc/thermview/README.md"
mkdir -p "$root/DEBIAN" "$work/debian"
cat > "$work/debian/control" <<CONTROL
Source: thermview
Section: graphics
Priority: optional
Maintainer: ThermView contributors <noreply@github.com>

Package: thermview
Architecture: $arch
Description: Radiometric thermal image and CSQ video viewer
CONTROL
# Resolve minimum libc/libstdc++ versions from the actual executable. GUI
# libraries loaded at runtime need explicit dependencies as well.
shlibs=$(cd "$work" && dpkg-shlibdeps -O -e"$root/usr/bin/thermview")
shlibs=${shlibs#shlibs:Depends=}
size=$(du -sk "$root/usr" | cut -f1)
cat > "$root/DEBIAN/control" <<CONTROL
Package: thermview
Version: $version
Architecture: $arch
Maintainer: ThermView contributors <noreply@github.com>
Section: graphics
Priority: optional
Homepage: https://github.com/alfiethology/thermview
Installed-Size: $size
Depends: $shlibs, libx11-6, libxcursor1, libxi6, libxrandr2, libxkbcommon0, libwayland-client0, libwayland-cursor0, libegl1, libvulkan1, libdbus-1-3
Recommends: shared-mime-info, hicolor-icon-theme, xdg-desktop-portal, xdg-desktop-portal-gtk | xdg-desktop-portal-kde | xdg-desktop-portal-gnome, zenity
Description: Radiometric thermal image and CSQ video viewer
 View thermal images and FLIR CSQ videos with colour palettes, calibration,
 per-frame temperature extrema, measurement points and a scrubbable timeline.
CONTROL
# Desktop, MIME and icon caches are refreshed by their owning packages' dpkg
# triggers; no privileged custom install or removal scripts are needed.
(cd "$root" && find usr -type f -print0 | LC_ALL=C sort -z | xargs -0 md5sum > DEBIAN/md5sums)
package="$out/thermview_${version}_${arch}.deb"
dpkg-deb --root-owner-group -Zxz --build "$root" "$package"
(cd "$out" && sha256sum "$(basename "$package")" > "$(basename "$package").sha256")
printf 'Built %s\n' "$package"
