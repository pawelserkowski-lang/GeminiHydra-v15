#!/usr/bin/env bash
# ============================================================================
# GeminiHydra v15 — PostgreSQL Database Backup Script
# ============================================================================
# Usage:
#   ./backup.sh                       # Uses defaults
#   ./backup.sh /path/to/backups      # Custom backup directory
#
# Cron example (daily at 2 AM):
#   0 2 * * * /path/to/GeminiHydra-v15/backend/scripts/backup.sh >> /var/log/gh-backup.log 2>&1
#
# Environment variables (override defaults):
#   PGHOST       — PostgreSQL host (default: localhost)
#   PGPORT       — PostgreSQL port (default: 5432)
#   PGUSER       — PostgreSQL user (default: postgres)
#   PGDATABASE   — Database name  (default: geminihydra)
#   BACKUP_DIR   — Backup directory (default: ./backups or $1)
#   KEEP_BACKUPS — Number of backups to retain (default: 7)
# ============================================================================

set -euo pipefail

# ── Configuration ────────────────────────────────────────────────────────────

DB_HOST="${PGHOST:-localhost}"
DB_PORT="${PGPORT:-5432}"
DB_USER="${PGUSER:-postgres}"
DB_NAME="${PGDATABASE:-geminihydra}"
BACKUP_DIR="${BACKUP_DIR:-${1:-$(dirname "$0")/../backups}}"
KEEP_BACKUPS="${KEEP_BACKUPS:-7}"

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="${BACKUP_DIR}/${DB_NAME}_${TIMESTAMP}.sql.gz"

# ── Colors (if terminal supports it) ────────────────────────────────────────

if [ -t 1 ]; then
  GREEN='\033[0;32m'
  RED='\033[0;31m'
  YELLOW='\033[1;33m'
  CYAN='\033[0;36m'
  NC='\033[0m'
else
  GREEN='' RED='' YELLOW='' CYAN='' NC=''
fi

log_info()  { echo -e "${CYAN}[INFO]${NC}  $(date '+%H:%M:%S') $*"; }
log_ok()    { echo -e "${GREEN}[OK]${NC}    $(date '+%H:%M:%S') $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $(date '+%H:%M:%S') $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $(date '+%H:%M:%S') $*"; }

# ── Pre-flight checks ───────────────────────────────────────────────────────

if ! command -v pg_dump &> /dev/null; then
  log_error "pg_dump not found in PATH. Install PostgreSQL client tools."
  exit 1
fi

if ! command -v gzip &> /dev/null; then
  log_error "gzip not found in PATH."
  exit 1
fi

# ── Create backup directory ─────────────────────────────────────────────────

mkdir -p "${BACKUP_DIR}"
log_info "Backup directory: ${BACKUP_DIR}"

# ── Dump database ───────────────────────────────────────────────────────────

log_info "Dumping ${DB_NAME}@${DB_HOST}:${DB_PORT} as ${DB_USER}..."

if pg_dump \
  --host="${DB_HOST}" \
  --port="${DB_PORT}" \
  --username="${DB_USER}" \
  --no-password \
  --format=plain \
  --verbose \
  --clean \
  --if-exists \
  "${DB_NAME}" 2>/dev/null | gzip > "${BACKUP_FILE}"; then

  FILESIZE=$(du -h "${BACKUP_FILE}" | cut -f1)
  log_ok "Backup created: ${BACKUP_FILE} (${FILESIZE})"
else
  log_error "pg_dump failed. Check credentials and connectivity."
  rm -f "${BACKUP_FILE}"
  exit 1
fi

# ── Rotate old backups (keep last N) ────────────────────────────────────────

BACKUP_COUNT=$(find "${BACKUP_DIR}" -name "${DB_NAME}_*.sql.gz" -type f | wc -l)

if [ "${BACKUP_COUNT}" -gt "${KEEP_BACKUPS}" ]; then
  REMOVE_COUNT=$((BACKUP_COUNT - KEEP_BACKUPS))
  log_info "Rotating backups: keeping ${KEEP_BACKUPS}, removing ${REMOVE_COUNT} oldest..."

  find "${BACKUP_DIR}" -name "${DB_NAME}_*.sql.gz" -type f -printf '%T@ %p\n' \
    | sort -n \
    | head -n "${REMOVE_COUNT}" \
    | cut -d' ' -f2- \
    | while IFS= read -r old_backup; do
        rm -f "${old_backup}"
        log_warn "Removed old backup: $(basename "${old_backup}")"
      done
else
  log_info "Backup count (${BACKUP_COUNT}) within retention limit (${KEEP_BACKUPS})."
fi

# ── Summary ──────────────────────────────────────────────────────────────────

log_ok "Backup complete. Total backups: $(find "${BACKUP_DIR}" -name "${DB_NAME}_*.sql.gz" -type f | wc -l)"
