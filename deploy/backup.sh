#!/usr/bin/env bash
set -euo pipefail

STACK_DIR="${STACK_DIR:-/opt/smbl}"
BACKUP_DIR="${BACKUP_DIR:-$STACK_DIR/backups}"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
KEEP_DAYS="${KEEP_DAYS:-14}"

install -d -m 700 "$BACKUP_DIR"

# Dumps the shared database. Product schemas can be restored individually.
docker compose -f "$STACK_DIR/compose.yaml" exec -T postgres \
  pg_dump --username postgres --dbname smbl --format=custom \
  > "$BACKUP_DIR/smbl-$STAMP.dump"

chmod 600 "$BACKUP_DIR/smbl-$STAMP.dump"
find "$BACKUP_DIR" -type f -name 'smbl-*.dump' -mtime +"$KEEP_DAYS" -delete

echo "backup written: $BACKUP_DIR/smbl-$STAMP.dump"
