#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
resolver="$root_dir/scripts/resolve-release-inputs.sh"
builder="$root_dir/scripts/build-release-package.sh"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

git init --bare -q "$work_dir/remote.git"
git init -q "$work_dir/source"
git -C "$work_dir/source" config user.email test@example.invalid
git -C "$work_dir/source" config user.name 'Release test'
printf '%s\n' fixture > "$work_dir/source/fixture.txt"
git -C "$work_dir/source" add fixture.txt
git -C "$work_dir/source" commit -qm 'fixture'
expected_sha="$(git -C "$work_dir/source" rev-parse HEAD)"
git -C "$work_dir/source" tag -a v1.2.3 -m v1.2.3
git -C "$work_dir/source" branch v9.9.9
git -C "$work_dir/source" remote add origin "$work_dir/remote.git"
git -C "$work_dir/source" push -q origin HEAD refs/tags/v1.2.3 v9.9.9
git init -q "$work_dir/resolution"
export RELEASE_GIT_DIR="$work_dir/resolution"

output="$work_dir/output"
RELEASE_EVENT_NAME=workflow_dispatch \
RELEASE_INPUT_TAG=v1.2.3 \
RELEASE_INPUT_MODE=release \
RELEASE_REF_TYPE=branch \
RELEASE_REF_NAME=main \
RELEASE_REPOSITORY="$work_dir/remote.git" \
GITHUB_OUTPUT="$output" \
  "$resolver"
grep -qx "sha=$expected_sha" "$output"
grep -qx 'tag=v1.2.3' "$output"
grep -qx 'version=1.2.3' "$output"
grep -qx 'mode=release' "$output"
git -C "$work_dir/resolution" cat-file -e "$expected_sha^{commit}"

marker="$work_dir/injected"
if RELEASE_EVENT_NAME=workflow_dispatch \
  RELEASE_INPUT_TAG="v1.2.3;touch $marker" \
  RELEASE_INPUT_MODE=release \
  RELEASE_REF_TYPE=branch \
  RELEASE_REF_NAME=main \
  RELEASE_REPOSITORY="$work_dir/remote.git" \
  GITHUB_OUTPUT="$output" \
    "$resolver"; then
  printf '%s\n' 'resolver accepted an injected tag' >&2
  exit 1
fi
test ! -e "$marker"

if RELEASE_EVENT_NAME=push \
  RELEASE_INPUT_TAG='' \
  RELEASE_INPUT_MODE='' \
  RELEASE_REF_TYPE=branch \
  RELEASE_REF_NAME=v9.9.9 \
  RELEASE_REPOSITORY="$work_dir/remote.git" \
  GITHUB_OUTPUT="$output" \
    "$resolver"; then
  printf '%s\n' 'resolver accepted a branch whose name looks like a tag' >&2
  exit 1
fi

if RELEASE_EVENT_NAME=workflow_dispatch \
  RELEASE_INPUT_TAG=v1.2.3 \
  RELEASE_INPUT_MODE='release;false' \
  RELEASE_REF_TYPE=branch \
  RELEASE_REF_NAME=main \
  RELEASE_REPOSITORY="$work_dir/remote.git" \
  GITHUB_OUTPUT="$output" \
    "$resolver"; then
  printf '%s\n' 'resolver accepted a package mode outside the allowlist' >&2
  exit 1
fi

printf '%s\n' '#!/usr/bin/env bash' 'printf "%s\\n" "$@" > "$BUILD_ARGS_LOG"' > "$work_dir/fake-builder"
chmod +x "$work_dir/fake-builder"
BUILD_ARGS_LOG="$work_dir/build-args" \
OBS_TELEGRAM_PACKAGE_BUILDER="$work_dir/fake-builder" \
  "$builder" release 1.2.3 /plugin /agent /telegram-bot-api
grep -qx -- '--release' "$work_dir/build-args"
grep -qx -- '--version' "$work_dir/build-args"
grep -qx -- '1.2.3' "$work_dir/build-args"

printf '%s\n' 'release input tests passed'
