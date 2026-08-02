#!/usr/bin/env bash
set -euo pipefail

: "${RELEASE_EVENT_NAME:?RELEASE_EVENT_NAME is required}"
: "${RELEASE_REF_TYPE:?RELEASE_REF_TYPE is required}"
: "${RELEASE_REF_NAME:?RELEASE_REF_NAME is required}"
: "${RELEASE_REPOSITORY:?RELEASE_REPOSITORY is required}"
: "${GITHUB_OUTPUT:?GITHUB_OUTPUT is required}"

case "$RELEASE_EVENT_NAME" in
  workflow_dispatch)
    tag="${RELEASE_INPUT_TAG:-}"
    mode="${RELEASE_INPUT_MODE:-}"
    ;;
  push)
    [[ "$RELEASE_REF_TYPE" == 'tag' ]] || {
      printf '%s\n' 'A push release must originate from a tag ref.' >&2
      exit 64
    }
    tag="$RELEASE_REF_NAME"
    mode='development'
    ;;
  *)
    printf 'Unsupported release event: %s\n' "$RELEASE_EVENT_NAME" >&2
    exit 64
    ;;
esac

[[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  printf '%s\n' 'Tag must match vMAJOR.MINOR.PATCH exactly.' >&2
  exit 64
}
case "$mode" in
  development|release) ;;
  *) printf '%s\n' 'Package mode must be development or release.' >&2; exit 64 ;;
esac

tag_ref="refs/tags/$tag"
resolution_git_dir="${RELEASE_GIT_DIR:-$PWD}"
git -C "$resolution_git_dir" rev-parse --git-dir >/dev/null
git -C "$resolution_git_dir" fetch --no-tags --depth 1 "$RELEASE_REPOSITORY" "$tag_ref"
resolved_sha="$(git -C "$resolution_git_dir" rev-parse 'FETCH_HEAD^{commit}')"
[[ "$resolved_sha" =~ ^[0-9a-f]{40}$ ]] || {
  printf 'Exact tag not found or did not resolve to a commit: %s\n' "$tag_ref" >&2
  exit 66
}

printf 'sha=%s\n' "$resolved_sha" >> "$GITHUB_OUTPUT"
printf 'tag=%s\n' "$tag" >> "$GITHUB_OUTPUT"
printf 'version=%s\n' "${tag#v}" >> "$GITHUB_OUTPUT"
printf 'mode=%s\n' "$mode" >> "$GITHUB_OUTPUT"
