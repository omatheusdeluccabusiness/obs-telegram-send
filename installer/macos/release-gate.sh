#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 || ( "$1" != '--development' && "$1" != '--release' ) ]]; then
  printf '%s\n' 'Uso: release-gate.sh --development|--release CAMINHO_DO_PKG' >&2
  exit 64
fi

mode="${1#--}"
package_path="$2"
[[ -f "$package_path" ]] || { printf 'Pacote não encontrado: %s\n' "$package_path" >&2; exit 66; }

sidecars="$(pkgutil --payload-files "$package_path" | grep -E '(^|/)\._' || true)"
if [[ -n "$sidecars" ]]; then
  if [[ "$mode" == 'release' ]]; then
    printf '%s\n%s\n' 'Release bloqueada: o payload contém sidecars AppleDouble (._*).' "$sidecars" >&2
    exit 1
  fi
  printf '%s\n%s\n' 'DEVELOPMENT ONLY — payload contém sidecars AppleDouble; este pacote não é publicável.' "$sidecars" >&2
fi
