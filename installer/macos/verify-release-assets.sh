#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 || ( "$1" != '--development' && "$1" != '--release' ) ]]; then
  printf '%s\n' 'Uso: verify-release-assets.sh --development|--release' >&2
  exit 64
fi

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mode="${1#--}"
required_assets=(
  'docs/images/macos/01-installer.pkg.png'
  'docs/images/macos/02-tools-menu.png'
  'docs/images/macos/03-onboarding.png'
  'docs/images/macos/04-send-confirmation.png'
)
missing=()
for asset in "${required_assets[@]}"; do
  [[ -f "$root_dir/$asset" ]] || missing+=("$asset")
done

if ((${#missing[@]})); then
  printf 'Capturas reais ausentes:\n%s\n' "$(printf '  %s\n' "${missing[@]}")" >&2
  if [[ "$mode" == 'release' ]]; then
    printf '%s\n' 'Release bloqueada até que todas as capturas acima sejam adicionadas.' >&2
    exit 1
  fi
  printf '%s\n' 'DEVELOPMENT ONLY — capturas pendentes; este pacote não é publicável.' >&2
fi
