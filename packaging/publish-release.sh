#!/usr/bin/env bash
# Publish existing build artifacts; safe to retry after a partial publication.
set -euo pipefail
tag=${1:?release tag required}
directory=${2:?artifact directory required}
notes=${3:?release notes required}
repo=${GITHUB_REPOSITORY:?GITHUB_REPOSITORY required}
shopt -s nullglob
assets=("$directory"/*)
[ "${#assets[@]}" -gt 0 ] || { echo 'No release artifacts found.' >&2; exit 1; }
[ -s "$notes" ] || { echo 'Release notes are empty or missing.' >&2; exit 1; }
for asset in "${assets[@]}"; do
    [ -f "$asset" ] || { echo "Not a release file: $asset" >&2; exit 1; }
done
if gh release view "$tag" --repo "$repo" >/dev/null 2>&1; then
    gh release edit "$tag" --repo "$repo" --title "$tag" --notes-file "$notes"
else
    gh release create "$tag" --repo "$repo" --verify-tag --title "$tag" --notes-file "$notes"
fi
gh release upload "$tag" "${assets[@]}" --repo "$repo" --clobber
