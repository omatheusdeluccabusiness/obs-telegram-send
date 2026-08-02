#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
script="$root_dir/installer/macos/scripts/postinstall"
test -x "$script"

temporary_dir="$(mktemp -d)"
trap 'rm -rf "$temporary_dir"' EXIT
OBS_TELEGRAM_POSTINSTALL_DRY_RUN=1 OBS_TELEGRAM_POSTINSTALL_LOG="$temporary_dir/calls" "$script"
grep -Fqx 'bootout gui/501 /Library/LaunchAgents/com.obs-telegram-send.agent.plist' "$temporary_dir/calls"
grep -Fqx 'bootstrap gui/501 /Library/LaunchAgents/com.obs-telegram-send.agent.plist' "$temporary_dir/calls"
grep -Fqx 'kickstart -k gui/501/com.obs-telegram-send.agent' "$temporary_dir/calls"

: > "$temporary_dir/calls"
OBS_TELEGRAM_POSTINSTALL_DRY_RUN=1 OBS_TELEGRAM_TEST_CONSOLE_USER=root \
  OBS_TELEGRAM_POSTINSTALL_LOG="$temporary_dir/calls" "$script"
! grep -q '^bootstrap ' "$temporary_dir/calls"
