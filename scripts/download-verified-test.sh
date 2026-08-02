#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
downloader="$root_dir/scripts/download-verified.sh"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

printf '%s' 'verified fixture' > "$work_dir/source"
good_sha='f9adb7d924ed98c558040c910600d7363d749e7d20e8d355626edd53b4fb929f'
"$downloader" "file://$work_dir/source" "$good_sha" "$work_dir/good"
cmp "$work_dir/source" "$work_dir/good"

if "$downloader" "file://$work_dir/source" \
  '0000000000000000000000000000000000000000000000000000000000000000' \
  "$work_dir/bad"; then
  printf '%s\n' 'downloader accepted a checksum mismatch' >&2
  exit 1
fi
test ! -e "$work_dir/bad"

printf '%s\n' 'verified download tests passed'
