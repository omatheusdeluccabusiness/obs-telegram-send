#!/usr/bin/env bash
set -euo pipefail

if (($# != 3)); then
  printf '%s\n' 'Usage: download-verified.sh URL SHA256 DESTINATION' >&2
  exit 64
fi

url="$1"
expected_sha="$2"
destination="$3"
[[ "$expected_sha" =~ ^[0-9a-f]{64}$ ]] || {
  printf '%s\n' 'Expected SHA-256 must contain exactly 64 lowercase hex characters.' >&2
  exit 64
}

mkdir -p "$(dirname "$destination")"
partial_path="$destination.partial.$$"
cleanup() {
  [[ ! -e "$partial_path" ]] || unlink "$partial_path"
}
trap cleanup EXIT

curl --fail --location --retry 3 --proto '=https,file' --tlsv1.2 \
  --output "$partial_path" "$url"
actual_sha="$(shasum -a 256 "$partial_path" | awk '{print $1}')"
if [[ "$actual_sha" != "$expected_sha" ]]; then
  printf 'SHA-256 mismatch for %s\nExpected: %s\nActual:   %s\n' \
    "$url" "$expected_sha" "$actual_sha" >&2
  exit 1
fi
mv -f "$partial_path" "$destination"
trap - EXIT
