#!/usr/bin/env bash
# Restore volumes from a backup archive created by scripts/backup.sh
set -euo pipefail

ARCHIVE="${1:?usage: restore.sh /path/to/upjs-gdd-YYYYMMDD-HHMMSS.tar.gz}"
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "${PROJECT_DIR}"

COMPOSE="${COMPOSE:-docker compose}"
TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT

tar -xzf "${ARCHIVE}" -C "${TMP}"
RESTORE_DIR="$(find "${TMP}" -maxdepth 1 -type d -name 'upjs-gdd-*' | head -1)"
if [[ -z "${RESTORE_DIR}" ]]; then
  echo "archive does not contain upjs-gdd-* directory" >&2
  exit 1
fi

echo "Stopping stack ..."
${COMPOSE} down || true

for vol in upjs-gdd-data upjs-gdd-games upjs-gdd-drafts; do
  TGZ="${RESTORE_DIR}/${vol}.tar.gz"
  if [[ ! -f "${TGZ}" ]]; then
    echo "skip missing ${vol}"
    continue
  fi
  echo "Restoring ${vol} ..."
  docker volume create "${vol}" >/dev/null 2>&1 || true
  docker run --rm \
    -v "${vol}:/data" \
    -v "${RESTORE_DIR}:/backup:ro" \
    alpine:3.20 \
    sh -c "rm -rf /data/* && tar xzf /backup/${vol}.tar.gz -C /data"
done

echo "Starting stack ..."
${COMPOSE} up -d
echo "Restore complete."
