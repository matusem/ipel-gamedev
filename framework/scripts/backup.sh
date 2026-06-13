#!/usr/bin/env bash
# Backup SQLite DB, games volume, and drafts volume from a running compose stack.
set -euo pipefail

BACKUP_ROOT="${1:-./backups}"
STAMP="$(date -u +%Y%m%d-%H%M%S)"
OUT_DIR="${BACKUP_ROOT}/upjs-gdd-${STAMP}"
ARCHIVE="${BACKUP_ROOT}/upjs-gdd-${STAMP}.tar.gz"

mkdir -p "${OUT_DIR}"

COMPOSE="${COMPOSE:-docker compose}"
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "${PROJECT_DIR}"

echo "Backing up to ${OUT_DIR} ..."

# Copy compose/env for restore reference
cp -a docker-compose.yml "${OUT_DIR}/" 2>/dev/null || true
cp -a .env "${OUT_DIR}/env.snapshot" 2>/dev/null || true

# Export named volumes via ephemeral container
for vol in upjs-gdd-data upjs-gdd-games upjs-gdd-drafts; do
  docker run --rm \
    -v "${vol}:/data:ro" \
    -v "${OUT_DIR}:/backup" \
    alpine:3.20 \
    sh -c "cd /data && tar czf /backup/${vol}.tar.gz ."
done

tar -czf "${ARCHIVE}" -C "${BACKUP_ROOT}" "upjs-gdd-${STAMP}"
rm -rf "${OUT_DIR}"

echo "Created ${ARCHIVE}"
