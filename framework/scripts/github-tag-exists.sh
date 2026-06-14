#!/usr/bin/env bash
# Check whether a git tag exists on GitHub (no gh CLI required).
set -euo pipefail

REPO="${1:?repo owner/name}"
TAG="${2:?tag name}"
TOKEN="${3:?github token}"

TAG="${TAG#refs/tags/}"
TAG="${TAG#refs/heads/}"

CODE="$(curl -sS -o /dev/null -w '%{http_code}' \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/${REPO}/git/ref/tags/${TAG}")"

if [ "$CODE" = "200" ]; then
  exit 0
fi

echo "Tag '${TAG}' not found on ${REPO} (HTTP ${CODE})" >&2
exit 1
