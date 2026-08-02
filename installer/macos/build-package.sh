#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
version="0.1.0"
mode=''
plugin_source="${OBS_TELEGRAM_PLUGIN_PATH:-$root_dir/build/plugin/obs-telegram-send.plugin}"
agent_source="${OBS_TELEGRAM_AGENT_PATH:-$root_dir/agent/target/release/obs-telegram-agent}"
bot_api_source="${OBS_TELEGRAM_BOT_API_PATH:-}"
developer_id_application="${OBS_TELEGRAM_DEVELOPER_ID_APPLICATION:-}"
developer_id_installer="${OBS_TELEGRAM_DEVELOPER_ID_INSTALLER:-}"
notary_profile="${OBS_TELEGRAM_NOTARY_PROFILE:-}"

usage() {
  cat <<'USAGE'
Uso:
  bash installer/macos/build-package.sh --development|--release --telegram-bot-api /caminho/telegram-bot-api [opções]

Modos obrigatórios:
  --development  Gera .pkg unsigned e binários ad-hoc. Não é publicável.
  --release      Exige Developer ID Application, Developer ID Installer e
                 perfil notarytool; assina, notariza, stapla e falha fechada.

Opções:
  --plugin CAMINHO                    Bundle obs-telegram-send.plugin compilado.
  --agent CAMINHO                     Executável obs-telegram-agent compilado.
  --telegram-bot-api CAMINHO          Executável oficial arm64 (obrigatório).
  --version VERSÃO                    Versão do pacote (padrão: 0.1.0).
  --developer-id-application NOME     Necessário em --release.
  --developer-id-installer NOME       Necessário em --release.
  --notary-profile PERFIL             Perfil keychain do notarytool em --release.

Variáveis equivalentes: OBS_TELEGRAM_PLUGIN_PATH, OBS_TELEGRAM_AGENT_PATH,
OBS_TELEGRAM_BOT_API_PATH, OBS_TELEGRAM_DEVELOPER_ID_APPLICATION,
OBS_TELEGRAM_DEVELOPER_ID_INSTALLER e OBS_TELEGRAM_NOTARY_PROFILE.
USAGE
}

while (($#)); do
  case "$1" in
    --development|--release) [[ -z "$mode" ]] || { printf 'Escolha apenas um modo.\n' >&2; exit 64; }; mode="${1#--}"; shift ;;
    --plugin) plugin_source="$2"; shift 2 ;;
    --agent) agent_source="$2"; shift 2 ;;
    --telegram-bot-api) bot_api_source="$2"; shift 2 ;;
    --version) version="$2"; shift 2 ;;
    --developer-id-application) developer_id_application="$2"; shift 2 ;;
    --developer-id-installer) developer_id_installer="$2"; shift 2 ;;
    --notary-profile) notary_profile="$2"; shift 2 ;;
    --help|-h) usage; exit 0 ;;
    *) printf 'Opção desconhecida: %s\n' "$1" >&2; usage >&2; exit 64 ;;
  esac
done

[[ -n "$mode" ]] || { printf 'Escolha --development ou --release.\n' >&2; exit 64; }
if [[ "$mode" == 'release' ]]; then
  [[ -n "$developer_id_application" ]] || { printf 'Release exige Developer ID Application.\n' >&2; exit 78; }
  [[ -n "$developer_id_installer" ]] || { printf 'Release exige Developer ID Installer.\n' >&2; exit 78; }
  [[ -n "$notary_profile" ]] || { printf 'Release exige perfil notarytool.\n' >&2; exit 78; }
fi
[[ -n "$bot_api_source" ]] || { printf 'Forneça --telegram-bot-api ou OBS_TELEGRAM_BOT_API_PATH.\n' >&2; exit 64; }

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
"$root_dir/installer/macos/verify-release-assets.sh" "--$mode"

dist_dir="$root_dir/dist"
payload_root="$dist_dir/root"
plugin_destination="$payload_root/Library/Application Support/obs-studio/plugins"
service_destination="$payload_root/Library/Application Support/OBS-Telegram-Send"
launch_agent_destination="$payload_root/Library/LaunchAgents"
scripts_dir="$root_dir/installer/macos/scripts"
if [[ "$mode" == 'release' ]]; then
  package_path="$dist_dir/OBS-Telegram-Send-macOS.pkg"
  unsigned_package="$dist_dir/OBS-Telegram-Send-macOS-unsigned.pkg"
else
  package_path="$dist_dir/OBS-Telegram-Send-macOS-development.pkg"
  unsigned_package="$package_path"
fi

rm -rf "$payload_root" "$unsigned_package" "$package_path"
mkdir -p "$plugin_destination" "$service_destination" "$launch_agent_destination"
ditto --norsrc --noextattr --noqtn --noacl \
  "$plugin_source" "$plugin_destination/obs-telegram-send.plugin"
install -m 0755 "$agent_source" "$service_destination/obs-telegram-agent"
install -m 0755 "$bot_api_source" "$service_destination/telegram-bot-api"
install -m 0644 "$root_dir/installer/macos/LaunchAgent.plist" \
  "$launch_agent_destination/com.obs-telegram-send.agent.plist"

if [[ "$mode" == 'release' ]]; then
  signing_identity="$developer_id_application"
  codesign_options=(--force --sign "$signing_identity" --options runtime --timestamp)
else
  signing_identity='-'
  codesign_options=(--force --sign "$signing_identity" --timestamp=none)
fi
codesign "${codesign_options[@]}" \
  "$plugin_destination/obs-telegram-send.plugin/Contents/MacOS/obs-telegram-send"
codesign "${codesign_options[@]}" --deep \
  "$plugin_destination/obs-telegram-send.plugin"
codesign "${codesign_options[@]}" "$service_destination/obs-telegram-agent"
codesign "${codesign_options[@]}" "$service_destination/telegram-bot-api"

pkgbuild \
  --root "$payload_root" \
  --scripts "$scripts_dir" \
  --identifier com.obs-telegram-send \
  --version "$version" \
  --install-location / \
  --ownership recommended \
  "$unsigned_package"

if [[ "$mode" == 'release' ]]; then
  productsign --sign "$developer_id_installer" "$unsigned_package" "$package_path"
  rm -f "$unsigned_package"
  xcrun notarytool submit "$package_path" --keychain-profile "$notary_profile" --wait
  xcrun stapler staple "$package_path"
  spctl --assess --type install --verbose=4 "$package_path"
fi

"$root_dir/installer/macos/release-gate.sh" "--$mode" "$package_path"
bash "$root_dir/installer/macos/package-layout-test.sh" "$package_path"
if [[ "$mode" == 'development' ]]; then
  printf 'DEVELOPMENT ONLY — não publicável, sem Developer ID nem notarização: %s\n' "$package_path"
else
  printf 'Release notarizada criada: %s\n' "$package_path"
fi
