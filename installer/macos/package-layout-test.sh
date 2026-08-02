#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
payload_root="$root_dir/dist/root"

test -x "$payload_root/Library/Application Support/obs-studio/plugins/obs-telegram-send.plugin/Contents/MacOS/obs-telegram-send"
test -x "$payload_root/Library/Application Support/OBS-Telegram-Send/obs-telegram-agent"
test -x "$payload_root/Library/Application Support/OBS-Telegram-Send/telegram-bot-api"
