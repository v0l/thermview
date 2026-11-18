#!/usr/bin/env bash
# Build on Arch Linux as a normal user. Packages the committed HEAD, not local edits.
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo"
for tool in makepkg git python3 sha256sum; do
    command -v "$tool" >/dev/null || { printf 'Missing build tool: %s\n' "$tool" >&2; exit 1; }
done
if [ "$(id -u)" -eq 0 ]; then
    echo 'Run the Arch build as a normal user, not root.' >&2
    exit 1
fi
if [ "$(uname -m)" != x86_64 ]; then
    echo 'The Arch package currently targets native x86_64 builds.' >&2
    exit 1
fi
version=$(git show HEAD:Cargo.toml | python3 -c 'import sys,tomllib; print(tomllib.loads(sys.stdin.read())["package"]["version"])')
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
git archive --format=tar.gz --prefix="thermview-$version/" HEAD > "$work/thermview-$version.tar.gz"
digest=$(sha256sum "$work/thermview-$version.tar.gz")
digest=${digest%% *}
sed -e "s/__VERSION__/$version/g" -e "s/__SOURCE_SHA256__/$digest/g" \
    packaging/arch/PKGBUILD.in > "$work/PKGBUILD"
mkdir -p "$repo/dist"
(cd "$work" && PKGDEST="$repo/dist" PKGEXT='.pkg.tar.zst' makepkg --cleanbuild --noconfirm)
printf 'Built %s/dist/thermview-%s-1-x86_64.pkg.tar.zst\n' "$repo" "$version"
