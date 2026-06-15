#!/usr/bin/env bash
#
# Cut a release: bump fortress_game's version, verify, commit, tag, and push.
# Pushing the tag triggers .github/workflows/release.yml, which builds the
# cross-platform binaries and uploads them to the GitHub Release.
#
# Usage:
#   ./release.sh <version>        e.g. ./release.sh 0.2.0  (or v0.2.0)
#
# Set RELEASE_SKIP_TESTS=1 to skip the verify step (not recommended).

set -euo pipefail
cd "$(dirname "$0")"

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
  echo "usage: $0 <version>   (e.g. $0 0.2.0)" >&2
  exit 1
fi

VERSION="${VERSION#v}"   # accept either 0.2.0 or v0.2.0
TAG="v$VERSION"

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: '$VERSION' is not a X.Y.Z version" >&2
  exit 1
fi

# A clean tree is non-negotiable: the tag must capture exactly what was tested.
# (Tagging with uncommitted edits is what sank v0.1.0.)
if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: working tree is dirty — commit or stash before releasing." >&2
  git status --short >&2
  exit 1
fi

if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "error: tag $TAG already exists." >&2
  exit 1
fi

echo "==> Bumping fortress_game to $VERSION"
cargo set-version --package fortress_game "$VERSION"

if [[ "${RELEASE_SKIP_TESTS:-0}" == "1" ]]; then
  echo "==> Skipping verify (RELEASE_SKIP_TESTS=1)"
else
  echo "==> Verifying (build + tests, locked)"
  cargo test --workspace --locked
fi

echo "==> Committing and tagging $TAG"
git commit -am "Release $TAG"
git tag -a "$TAG" -m "Release $TAG"

echo "==> Pushing branch and tag"
git push origin HEAD
git push origin "$TAG"

echo
echo "Done — $TAG pushed. The release build is running:"
echo "  https://github.com/bhex/adventure_fortress/actions"
