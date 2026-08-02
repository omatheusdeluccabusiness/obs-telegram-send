#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
gate="$root_dir/installer/macos/release-gate.sh"
test -x "$gate"
grep -F -- '--options runtime' "$root_dir/installer/macos/build-package.sh" >/dev/null

temporary_dir="$(mktemp -d)"
trap 'rm -rf "$temporary_dir"' EXIT
mkdir -p "$temporary_dir/bin"
cat > "$temporary_dir/bin/pkgutil" <<'SCRIPT'
#!/usr/bin/env bash
printf '%s\n' './Library/Application Support/OBS-Telegram-Send/._telegram-bot-api'
SCRIPT
chmod +x "$temporary_dir/bin/pkgutil"
touch "$temporary_dir/package.pkg"

PATH="$temporary_dir/bin:$PATH" "$gate" --development "$temporary_dir/package.pkg"
if PATH="$temporary_dir/bin:$PATH" "$gate" --release "$temporary_dir/package.pkg"; then
  printf '%s\n' 'release gate accepted a package with AppleDouble sidecars' >&2
  exit 1
fi
