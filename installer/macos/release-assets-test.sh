#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
verifier="$root_dir/installer/macos/verify-release-assets.sh"
test -x "$verifier"

"$verifier" --development
if "$verifier" --release; then
  printf '%s\n' 'release asset verifier accepted missing screenshots' >&2
  exit 1
fi
