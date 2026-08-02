#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
version="0.1.0"
plugin_source="${OBS_TELEGRAM_PLUGIN_PATH:-$root_dir/build/plugin/obs-telegram-send.plugin}"
agent_source="${OBS_TELEGRAM_AGENT_PATH:-$root_dir/agent/target/release/obs-telegram-agent}"
bot_api_source="${OBS_TELEGRAM_BOT_API_PATH:-}"

usage() {
  cat <<'USAGE'
Uso:
  bash installer/macos/build-package.sh --telegram-bot-api /caminho/telegram-bot-api [opções]

Opções:
  --plugin CAMINHO            Bundle obs-telegram-send.plugin já compilado.
  --agent CAMINHO             Executável obs-telegram-agent já compilado.
  --telegram-bot-api CAMINHO  Executável oficial arm64 telegram-bot-api (obrigatório).
  --version VERSÃO            Versão do pacote (padrão: 0.1.0).

Também aceita OBS_TELEGRAM_PLUGIN_PATH, OBS_TELEGRAM_AGENT_PATH e
OBS_TELEGRAM_BOT_API_PATH. O binário oficial não é versionado no repositório:
ele é apenas copiado para o payload durante esta etapa.
USAGE
}

while (($#)); do
  case "$1" in
    --plugin) plugin_source="$2"; shift 2 ;;
    --agent) agent_source="$2"; shift 2 ;;
    --telegram-bot-api) bot_api_source="$2"; shift 2 ;;
    --version) version="$2"; shift 2 ;;
    --help|-h) usage; exit 0 ;;
    *) printf 'Opção desconhecida: %s\n' "$1" >&2; usage >&2; exit 64 ;;
  esac
done

if [[ -z "$bot_api_source" ]]; then
  printf '%s\n' 'Forneça --telegram-bot-api ou OBS_TELEGRAM_BOT_API_PATH.' >&2
  exit 64
fi

require_arm64_executable() {
  local label="$1" path="$2"
  [[ -f "$path" && -x "$path" ]] || { printf '%s não é executável: %s\n' "$label" "$path" >&2; exit 66; }
  file "$path" | grep -q 'arm64' || { printf '%s não é arm64: %s\n' "$label" "$path" >&2; exit 65; }
}

require_arm64_executable 'Agente' "$agent_source"
require_arm64_executable 'Servidor Telegram Bot API' "$bot_api_source"
plugin_executable="$plugin_source/Contents/MacOS/obs-telegram-send"
[[ -d "$plugin_source" ]] || { printf 'Plugin não encontrado: %s\n' "$plugin_source" >&2; exit 66; }
require_arm64_executable 'Plugin' "$plugin_executable"

dist_dir="$root_dir/dist"
payload_root="$dist_dir/root"
plugin_destination="$payload_root/Library/Application Support/obs-studio/plugins"
service_destination="$payload_root/Library/Application Support/OBS-Telegram-Send"
launch_agent_destination="$payload_root/Library/LaunchAgents"

rm -rf "$payload_root" "$dist_dir/scripts" "$dist_dir/OBS-Telegram-Send-macOS.pkg"
mkdir -p "$plugin_destination" "$service_destination" "$launch_agent_destination"
ditto --norsrc --noextattr --noqtn --noacl \
  "$plugin_source" "$plugin_destination/obs-telegram-send.plugin"
install -m 0755 "$agent_source" "$service_destination/obs-telegram-agent"
install -m 0755 "$bot_api_source" "$service_destination/telegram-bot-api"
install -m 0644 "$root_dir/installer/macos/LaunchAgent.plist" \
  "$launch_agent_destination/com.obs-telegram-send.agent.plist"

# Ad-hoc signatures make the local artifacts self-consistent without claiming
# a Developer ID identity or notarization. The .pkg itself remains unsigned.
codesign --force --sign - --timestamp=none \
  "$plugin_destination/obs-telegram-send.plugin/Contents/MacOS/obs-telegram-send"
codesign --force --deep --sign - --timestamp=none \
  "$plugin_destination/obs-telegram-send.plugin"
codesign --force --sign - --timestamp=none "$service_destination/obs-telegram-agent"
codesign --force --sign - --timestamp=none "$service_destination/telegram-bot-api"
# Codex/macOS can add provenance attributes while constructing the payload.
# Remove metadata before pkgbuild so it cannot emit AppleDouble sidecar files.
xattr -cr "$payload_root"

pkgbuild \
  --root "$payload_root" \
  --identifier com.obs-telegram-send \
  --version "$version" \
  --install-location / \
  --ownership recommended \
  "$dist_dir/OBS-Telegram-Send-macOS.pkg"

bash "$root_dir/installer/macos/package-layout-test.sh"
printf 'Pacote criado (não notarizado): %s\n' "$dist_dir/OBS-Telegram-Send-macOS.pkg"
