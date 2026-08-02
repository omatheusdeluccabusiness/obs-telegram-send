#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
publisher="$root_dir/scripts/publish-draft-release.sh"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

git init --bare -q "$work_dir/remote.git"
git init -q "$work_dir/source"
git -C "$work_dir/source" config user.email test@example.invalid
git -C "$work_dir/source" config user.name 'Publish test'
printf '%s\n' first > "$work_dir/source/fixture.txt"
git -C "$work_dir/source" add fixture.txt
git -C "$work_dir/source" commit -qm first
expected_sha="$(git -C "$work_dir/source" rev-parse HEAD)"
git -C "$work_dir/source" tag v1.2.3
printf '%s\n' second > "$work_dir/source/fixture.txt"
git -C "$work_dir/source" commit -qam second
moved_sha="$(git -C "$work_dir/source" rev-parse HEAD)"
git -C "$work_dir/source" remote add origin "$work_dir/remote.git"
git -C "$work_dir/source" push -q origin refs/tags/v1.2.3
git init -q "$work_dir/publish-git"
touch "$work_dir/package.pkg"

printf '%s\n' '#!/usr/bin/env bash' \
  'set -euo pipefail' \
  'printf "%s\\n" "$*" >> "$FAKE_GH_LOG"' \
  'if [[ "$1 $2" == "release view" ]]; then' \
  '  [[ "${FAKE_GH_RELEASE_EXISTS:-false}" == true ]] || exit 1' \
  '  if [[ "$*" == *"--json isDraft"* ]]; then printf "%s\\n" true; fi' \
  'fi' > "$work_dir/fake-gh"
chmod +x "$work_dir/fake-gh"

publish() {
  TAG=v1.2.3 \
  EXPECTED_SHA="$expected_sha" \
  RELEASE_REPOSITORY="$work_dir/remote.git" \
  RELEASE_GIT_DIR="$work_dir/publish-git" \
  GH_CLI="$work_dir/fake-gh" \
  FAKE_GH_LOG="$work_dir/gh.log" \
  FAKE_GH_RELEASE_EXISTS="${FAKE_GH_RELEASE_EXISTS:-false}" \
    "$publisher" "$work_dir/package.pkg" 'draft notes'
}

# A stable exact tag may create a draft, but creation must ask GitHub to verify
# the already-existing tag rather than synthesizing one from a branch.
: > "$work_dir/gh.log"
FAKE_GH_RELEASE_EXISTS=false publish
grep -Fqx 'release create v1.2.3'" $work_dir/package.pkg "'--draft --verify-tag --title v1.2.3 --notes draft notes' "$work_dir/gh.log"

# Moving the tag after the original resolution blocks a new draft.
git -C "$work_dir/source" tag -f v1.2.3 "$moved_sha"
git -C "$work_dir/source" push -q --force origin refs/tags/v1.2.3
: > "$work_dir/gh.log"
if FAKE_GH_RELEASE_EXISTS=false publish; then
  printf '%s\n' 'publisher accepted a moved tag before create' >&2
  exit 1
fi
! grep -Fq 'release create' "$work_dir/gh.log"

# A removed tag cannot be recreated implicitly by gh release create.
git -C "$work_dir/source" push -q --delete origin refs/tags/v1.2.3
: > "$work_dir/gh.log"
if FAKE_GH_RELEASE_EXISTS=false publish; then
  printf '%s\n' 'publisher accepted a removed tag' >&2
  exit 1
fi
! grep -Fq 'release create' "$work_dir/gh.log"

# An existing draft also fails closed when its tag no longer resolves to the
# immutable commit selected at the beginning of the workflow.
git -C "$work_dir/source" tag -f v1.2.3 "$moved_sha"
git -C "$work_dir/source" push -q origin refs/tags/v1.2.3
: > "$work_dir/gh.log"
if FAKE_GH_RELEASE_EXISTS=true publish; then
  printf '%s\n' 'publisher uploaded to a draft whose tag commit mismatched' >&2
  exit 1
fi
! grep -Fq 'release upload' "$work_dir/gh.log"

printf '%s\n' 'draft publish tests passed'
