#!/bin/bash
# Release the current branch: push, PR, wait for CI, merge, tag, wait for the
# release build. The version and CHANGELOG section must already be committed.
#   tools/release.sh
# Network steps retry (the link to GitHub drops often). Waits are 12 minutes.
set -u
cd "$(dirname "$0")/.." || exit 1
REPO=lDOCI/rivet
WAIT=$((12 * 60))
say()  { printf '\n\033[1m== %s\033[0m\n' "$*"; }
die()  { printf '\033[31mStop: %s\033[0m\n' "$*"; exit 1; }
retry() { for n in 1 2 3 4 5 6; do "$@" && return 0; echo "network, retry $n…"; sleep 10; done; return 1; }

BRANCH=$(git branch --show-current)
[[ -n "$BRANCH" && "$BRANCH" != main ]] || die "on main; check out the release branch"
[[ -z "$(git status --porcelain)" ]] || die "uncommitted changes"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
TAG="v$VERSION-alpha"
grep -q "^## $TAG\$" CHANGELOG.md || die "CHANGELOG.md has no section $TAG"
echo "branch $BRANCH, releasing $TAG"

say "Push"
retry git push -q -u origin "$BRANCH" || die "push failed"

say "PR"
if ! retry gh pr view "$BRANCH" --repo "$REPO" --json state -q .state 2>/dev/null | grep -q OPEN; then
  retry gh pr create --repo "$REPO" --base main --head "$BRANCH" --fill || die "PR not opened"
fi
gh pr view "$BRANCH" --repo "$REPO" --json url -q .url

say "CI (up to 12 min)"
END=$(( $(date +%s) + WAIT )); sleep 20
while :; do
  OUT=$(gh pr checks "$BRANCH" --repo "$REPO" 2>/dev/null)
  if [[ -n "$OUT" ]]; then
    echo "$OUT" | grep -qw fail && { echo "$OUT"; die "CI failed, nothing merged"; }
    echo "$OUT" | grep -qwE 'pending|queued|in_progress' || { echo "$OUT"; break; }
  fi
  (( $(date +%s) < END )) || die "CI still running after 12 min; run again later"
  sleep 30
done

say "Merge"
retry gh pr merge "$BRANCH" --repo "$REPO" --merge || die "merge failed"
retry git fetch -q origin || die "fetch failed"

say "Tag $TAG"
git rev-parse -q --verify "refs/tags/$TAG" >/dev/null || git tag "$TAG" origin/main || die "tag failed"
retry git push -q origin "$TAG" || die "tag push failed (it exists locally: git push origin $TAG)"

say "Release build (up to 12 min)"
END=$(( $(date +%s) + WAIT )); RUN=""
until [[ -n "$RUN" ]]; do
  (( $(date +%s) < END )) || die "no release run found; see Actions on GitHub"
  sleep 20
  RUN=$(gh run list --repo "$REPO" --workflow release.yml --branch "$TAG" -L1 --json databaseId -q '.[0].databaseId' 2>/dev/null)
done
until [[ "$(gh run view "$RUN" --repo "$REPO" --json status -q .status 2>/dev/null)" == completed ]]; do
  (( $(date +%s) < END )) || die "release build over 12 min: https://github.com/$REPO/actions/runs/$RUN"
  sleep 30
done
[[ "$(retry gh run view "$RUN" --repo "$REPO" --json conclusion -q .conclusion)" == success ]] \
  || die "release build failed: https://github.com/$REPO/actions/runs/$RUN"
echo "assets: $(retry gh release view "$TAG" --repo "$REPO" --json assets -q '[.assets[].name] | join(" ")')"
echo "latest: $(retry gh api "repos/$REPO/releases/latest" -q .tag_name)"
printf '\033[32mDone: https://github.com/%s/releases/tag/%s\033[0m\n' "$REPO" "$TAG"
