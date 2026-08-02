#!/usr/bin/env bash
set -euo pipefail

if (($# != 5)); then
  printf '%s\n' 'Usage: build-release-package.sh MODE VERSION PLUGIN AGENT TELEGRAM_BOT_API' >&2
  exit 64
fi

mode="$1"
version="$2"
plugin_path="$3"
agent_path="$4"
telegram_bot_api_path="$5"
case "$mode" in
  development|release) ;;
  *) printf '%s\n' 'Package mode must be development or release.' >&2; exit 64 ;;
esac
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  printf '%s\n' 'Package version must be MAJOR.MINOR.PATCH.' >&2
  exit 64
}

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
package_builder="${OBS_TELEGRAM_PACKAGE_BUILDER:-$root_dir/installer/macos/build-package.sh}"
exec "$package_builder" "--$mode" \
  --version "$version" \
  --plugin "$plugin_path" \
  --agent "$agent_path" \
  --telegram-bot-api "$telegram_bot_api_path"
