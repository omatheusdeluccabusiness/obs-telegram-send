#!/usr/bin/env bash
set -euo pipefail

root_dir="${OBS_TELEGRAM_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
dist_dir="${OBS_TELEGRAM_DIST_DIR:-$root_dir/dist}"
mode='development'

usage() {
  printf '%s\n' 'Uso: verify-release.sh [--development|--release]'
}

if (($# == 1)); then
  case "$1" in
    --development|--release) mode="${1#--}" ;;
    --help|-h) usage; exit 0 ;;
    *) usage >&2; exit 64 ;;
  esac
elif (($# != 0)); then
  usage >&2
  exit 64
fi

if [[ "$mode" == 'release' ]]; then
  package_path="$dist_dir/OBS-Telegram-Send-macOS.pkg"
else
  package_path="$dist_dir/OBS-Telegram-Send-macOS-development.pkg"
fi

cargo test --manifest-path "$root_dir/agent/Cargo.toml"
if command -v ctest >/dev/null 2>&1; then
  ctest --test-dir "$root_dir/build" --output-on-failure
elif [[ "$mode" == 'development' && -f "$root_dir/build/Makefile" ]]; then
  # The local developer image may retain a configured Makefile after CMake has
  # been removed. Keep the development verification usable without weakening
  # the release runner, which must have the explicit CTest command available.
  make -C "$root_dir/build" test
else
  printf '%s\n' 'ctest é obrigatório para verificar um pacote de release.' >&2
  exit 69
fi
[[ -f "$package_path" ]] || {
  printf 'Pacote %s ausente: %s\n' "$mode" "$package_path" >&2
  exit 66
}

if [[ "$mode" == 'development' ]]; then
  printf 'DEVELOPMENT ONLY — verificado, mas não publicável: %s\n' "$package_path"
  exit 0
fi

release_gate="$root_dir/installer/macos/release-gate.sh"
[[ -x "$release_gate" ]] || {
  printf 'Release gate ausente ou não executável: %s\n' "$release_gate" >&2
  exit 66
}
"$release_gate" --release "$package_path"

# A public filename by itself is not evidence of a public-ready package.
# Require both a Developer ID installer signature and macOS trust assessment.
signature_info="$(pkgutil --check-signature "$package_path")"
printf '%s\n' "$signature_info"
if ! grep -Eq 'Developer ID Installer:' <<<"$signature_info"; then
  printf '%s\n' 'Release bloqueada: assinatura Developer ID Installer não encontrada.' >&2
  exit 1
fi
spctl --assess --type install --verbose=4 "$package_path"

# In a checkout, reject an artifact older than the source revision. This avoids
# validating a stale public-named package left by an earlier build.
if git -C "$root_dir" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  source_epoch="$(git -C "$root_dir" log -1 --format=%ct)"
  package_epoch="$(stat -f %m "$package_path")"
  if ((package_epoch < source_epoch)); then
    printf '%s\n' 'Release bloqueada: pacote é mais antigo que a revisão atual.' >&2
    exit 1
  fi
fi

printf 'Release assinada e notarizada verificada: %s\n' "$package_path"
