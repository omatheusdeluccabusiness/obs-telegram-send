#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 || ( "$1" != '--development' && "$1" != '--release' ) ]]; then
  printf '%s\n' 'Uso: release-gate.sh --development|--release CAMINHO_DO_PKG' >&2
  exit 64
fi

mode="${1#--}"
package_path="$2"
[[ -f "$package_path" ]] || { printf 'Pacote não encontrado: %s\n' "$package_path" >&2; exit 66; }

payload_sidecars="$(pkgutil --payload-files "$package_path" | grep -E '(^|/)\._' || true)"
expanded_parent="$(mktemp -d)"
expanded_package="$expanded_parent/package"
trap 'rm -rf "$expanded_parent"' EXIT
pkgutil --expand "$package_path" "$expanded_package"
archive_sidecars="$(find "$expanded_package" -name '._*' -print || true)"
sidecars="$(printf '%s\n%s\n' "$payload_sidecars" "$archive_sidecars" | sed '/^$/d')"
if [[ -n "$sidecars" ]]; then
  if [[ "$mode" == 'release' ]]; then
    printf '%s\n%s\n' 'Release bloqueada: o pacote contém sidecars AppleDouble (._*).' "$sidecars" >&2
    exit 1
  fi
  printf '%s\n%s\n' 'DEVELOPMENT ONLY — pacote contém sidecars AppleDouble; este pacote não é publicável.' "$sidecars" >&2
fi
