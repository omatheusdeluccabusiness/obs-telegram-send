#!/usr/bin/env bash
set -euo pipefail

if (($# != 2)); then
  printf '%s\n' 'Usage: publish-draft-release.sh PACKAGE NOTES' >&2
  exit 64
fi

package_path="$1"
notes="$2"
: "${TAG:?TAG is required}"
: "${EXPECTED_SHA:?EXPECTED_SHA is required}"
: "${RELEASE_REPOSITORY:?RELEASE_REPOSITORY is required}"
[[ -f "$package_path" ]] || { printf 'Package not found: %s\n' "$package_path" >&2; exit 66; }
[[ "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  printf '%s\n' 'Tag must match vMAJOR.MINOR.PATCH exactly.' >&2
  exit 64
}
[[ "$EXPECTED_SHA" =~ ^[0-9a-f]{40}$ ]] || {
  printf '%s\n' 'EXPECTED_SHA must be a full SHA-1 commit ID.' >&2
  exit 64
}

release_git_dir="${RELEASE_GIT_DIR:-$PWD}"
gh_cli="${GH_CLI:-gh}"
git -C "$release_git_dir" rev-parse --git-dir >/dev/null

revalidate_exact_tag() {
  local tag_ref="refs/tags/$TAG" current_sha
  if ! git -C "$release_git_dir" fetch --no-tags --depth 1 "$RELEASE_REPOSITORY" "$tag_ref"; then
    printf 'Release blocked: exact tag is missing: %s\n' "$tag_ref" >&2
    return 1
  fi
  current_sha="$(git -C "$release_git_dir" rev-parse 'FETCH_HEAD^{commit}')"
  if [[ "$current_sha" != "$EXPECTED_SHA" ]]; then
    printf 'Release blocked: tag moved from %s to %s.\n' "$EXPECTED_SHA" "$current_sha" >&2
    return 1
  fi
}

if "$gh_cli" release view "$TAG" >/dev/null 2>&1; then
  [[ "$("$gh_cli" release view "$TAG" --json isDraft --jq .isDraft)" == 'true' ]] || {
    printf '%s\n' 'Refusing to upload a release package to a non-draft GitHub Release.' >&2
    exit 1
  }
  revalidate_exact_tag
  "$gh_cli" release upload "$TAG" "$package_path" --clobber
else
  revalidate_exact_tag
  "$gh_cli" release create "$TAG" "$package_path" \
    --draft --verify-tag --title "$TAG" --notes "$notes"
fi
