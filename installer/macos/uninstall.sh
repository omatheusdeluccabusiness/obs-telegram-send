#!/usr/bin/env bash
set -euo pipefail

plugin_path='/Library/Application Support/obs-studio/plugins/obs-telegram-send.plugin'
service_path='/Library/Application Support/OBS-Telegram-Send'
launch_agent='/Library/LaunchAgents/com.obs-telegram-send.agent.plist'

if [[ "$(id -u)" -ne 0 ]]; then
  printf '%s\n' 'Execute novamente com: sudo bash installer/macos/uninstall.sh' >&2
  exit 1
fi

console_user="$(stat -f '%Su' /dev/console)"
if [[ "$console_user" != 'root' && "$console_user" != 'loginwindow' ]]; then
  console_uid="$(id -u "$console_user")"
  launchctl bootout "gui/$console_uid" "$launch_agent" 2>/dev/null || true
fi

rm -rf "$plugin_path"
rm -rf "$service_path"
rm -f "$launch_agent"

cat <<'MESSAGE'
OBS Telegram Send foi removido destes locais:
  /Library/Application Support/obs-studio/plugins/obs-telegram-send.plugin
  /Library/Application Support/OBS-Telegram-Send
  /Library/LaunchAgents/com.obs-telegram-send.agent.plist

Gravações não foram removidas. O Keychain e o estado por usuário também foram
preservados: veja docs/troubleshooting-pt-BR.md se quiser apagá-los manualmente.
MESSAGE
