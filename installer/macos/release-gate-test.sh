#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
gate="$root_dir/installer/macos/release-gate.sh"
test -x "$gate"
grep -F -- '--options runtime' "$root_dir/installer/macos/build-package.sh" >/dev/null
pkgbuild_line="$(grep -n '^pkgbuild' "$root_dir/installer/macos/build-package.sh" | cut -d: -f1)"
gate_line="$(grep -n 'release-gate.sh.*--release.*unsigned_package' "$root_dir/installer/macos/build-package.sh" | cut -d: -f1)"
productsign_line="$(grep -n '^  productsign' "$root_dir/installer/macos/build-package.sh" | cut -d: -f1)"
spctl_line="$(grep -n '^  spctl ' "$root_dir/installer/macos/build-package.sh" | cut -d: -f1)"
signed_layout_line="$(grep -n 'package-layout-test.sh.*signed_package' "$root_dir/installer/macos/build-package.sh" | cut -d: -f1)"
move_line="$(grep -n '^  mv -f .*signed_package.*package_path' "$root_dir/installer/macos/build-package.sh" | cut -d: -f1)"
test "$pkgbuild_line" -lt "$gate_line"
test "$gate_line" -lt "$productsign_line"
test "$spctl_line" -lt "$signed_layout_line"
test "$signed_layout_line" -lt "$move_line"

temporary_dir="$(mktemp -d)"
trap 'rm -rf "$temporary_dir"' EXIT
mkdir -p "$temporary_dir/bin"
cat > "$temporary_dir/bin/pkgutil" <<'SCRIPT'
#!/usr/bin/env bash
case "$1" in
  --payload-files)
    printf '%s\n' './Library/Application Support/OBS-Telegram-Send/telegram-bot-api'
    ;;
  --expand)
    mkdir -p "$3/Scripts"
    touch "$3/Scripts/._postinstall"
    ;;
esac
SCRIPT
chmod +x "$temporary_dir/bin/pkgutil"
touch "$temporary_dir/package.pkg"

PATH="$temporary_dir/bin:$PATH" "$gate" --development "$temporary_dir/package.pkg"
if PATH="$temporary_dir/bin:$PATH" "$gate" --release "$temporary_dir/package.pkg"; then
  printf '%s\n' 'release gate accepted a package with AppleDouble sidecars' >&2
  exit 1
fi
