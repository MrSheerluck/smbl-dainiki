#!/usr/bin/env bash
set -euo pipefail

STACK_DIR="${STACK_DIR:-/opt/smbl}"
BACKUP_DIR="${BACKUP_DIR:-$STACK_DIR/backups}"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
KEEP_DAYS="${KEEP_DAYS:-14}"

if [ -f "$STACK_DIR/backup.env" ]; then
  set -a
  # shellcheck disable=SC1090
  . "$STACK_DIR/backup.env"
  set +a
  if [ -n "${R2_ACCESS_KEY_ID:-}" ] && [ -n "${R2_SECRET_ACCESS_KEY:-}" ]; then
    export AWS_ACCESS_KEY_ID="$R2_ACCESS_KEY_ID"
    export AWS_SECRET_ACCESS_KEY="$R2_SECRET_ACCESS_KEY"
    export AWS_DEFAULT_REGION="${R2_REGION:-auto}"
  fi
fi

install -d -m 700 "$BACKUP_DIR"

# Dumps the shared database. Product schemas can be restored individually.
docker compose -f "$STACK_DIR/compose.yaml" exec -T postgres \
  pg_dump --username postgres --dbname smbl --format=custom \
  > "$BACKUP_DIR/smbl-$STAMP.dump"

chmod 600 "$BACKUP_DIR/smbl-$STAMP.dump"

if [ -n "${R2_BACKUP_BUCKET:-}" ] && [ -n "${R2_ENDPOINT:-}" ] && command -v aws >/dev/null 2>&1; then
  aws --endpoint-url "$R2_ENDPOINT" s3 cp     "$BACKUP_DIR/smbl-$STAMP.dump"     "s3://$R2_BACKUP_BUCKET/backups/smbl-$STAMP.dump"     --only-show-errors
  echo "offsite backup written: s3://$R2_BACKUP_BUCKET/backups/smbl-$STAMP.dump"
else
  echo "offsite backup skipped: R2_BACKUP_BUCKET/R2_ENDPOINT/aws not configured"
fi

find "$BACKUP_DIR" -type f -name 'smbl-*.dump' -mtime +"$KEEP_DAYS" -delete

echo "backup written: $BACKUP_DIR/smbl-$STAMP.dump"
