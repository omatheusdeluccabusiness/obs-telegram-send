#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
verifier="$root_dir/scripts/verify-release.sh"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

make_fake_tool() {
  local name="$1"
  cat >"$work_dir/bin/$name" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
exit 0
SH
  chmod +x "$work_dir/bin/$name"
}

mkdir -p "$work_dir/bin" "$work_dir/repo/dist"
make_fake_tool cargo
make_fake_tool ctest
make_fake_tool stat

cat >"$work_dir/bin/pkgutil" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  --expand) mkdir -p "$3" ;;
  --check-signature) printf '%s\n' '1. Developer ID Installer: Example (TEAMID)' ;;
esac
SH
chmod +x "$work_dir/bin/pkgutil"

cat >"$work_dir/bin/spctl" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' 'rejected: development package'
exit 1
SH
chmod +x "$work_dir/bin/spctl"

# A development build has a deliberately distinct filename and must never be
# confused with the public release filename.
touch "$work_dir/repo/dist/OBS-Telegram-Send-macOS-development.pkg"
if ! PATH="$work_dir/bin:$PATH" OBS_TELEGRAM_DIST_DIR="$work_dir/repo/dist" "$verifier" --development; then
  printf '%s\n' 'development verification rejected the development package' >&2
  exit 1
fi

# A release verification must not accept a development-only package renamed to
# the public filename: the package must pass the release gate and macOS trust
# assessment.
mv "$work_dir/repo/dist/OBS-Telegram-Send-macOS-development.pkg" \
  "$work_dir/repo/dist/OBS-Telegram-Send-macOS.pkg"
if PATH="$work_dir/bin:$PATH" OBS_TELEGRAM_DIST_DIR="$work_dir/repo/dist" "$verifier" --release; then
  printf '%s\n' 'release verification accepted an unsigned public package' >&2
  exit 1
fi

printf '%s\n' 'verify-release tests passed'
