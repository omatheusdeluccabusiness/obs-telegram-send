#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
payload_root="$root_dir/dist/root"
package_path="${1:-$root_dir/dist/OBS-Telegram-Send-macOS-development.pkg}"

test -x "$payload_root/Library/Application Support/obs-studio/plugins/obs-telegram-send.plugin/Contents/MacOS/obs-telegram-send"
test -x "$payload_root/Library/Application Support/OBS-Telegram-Send/obs-telegram-agent"
test -x "$payload_root/Library/Application Support/OBS-Telegram-Send/telegram-bot-api"
test -x "$root_dir/installer/macos/scripts/postinstall"
test -f "$package_path"

expanded_parent="$(mktemp -d)"
expanded_package="$expanded_parent/package"
trap 'rm -rf "$expanded_parent"' EXIT
pkgutil --expand "$package_path" "$expanded_package"
test -x "$expanded_package/Scripts/postinstall"
