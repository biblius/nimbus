#!/usr/bin/env bash
# Export Nimbus data from Postgres and import into SQLite (CSV + type remaps).
#
# Usage:
#   ./scripts/pg2sqlite.sh <postgres-url> <sqlite-db-path>
#
# Prerequisites:
#   - Target SQLite database already has the Nimbus schema
#     (sqlx migrate run --source server/migrations/sqlite).
#   - Target tables must be empty.
#   - Copy UPLOAD_DIR (file BLOBs) separately; this script moves DB rows only.
#
# Example:
#   ./scripts/pg2sqlite.sh \
#     'postgres://nimbus:nimbus@127.0.0.1:5434/nimbus' \
#     ./data/nimbus.db
set -euo pipefail

usage() {
  sed -n '2,16p' "$0" | sed 's/^# \?//'
  exit 1
}

[[ $# -eq 2 ]] || usage

PG_URL=$1
SQLITE_DB=$2

command -v sqlite3 >/dev/null || {
  echo "sqlite3 is required" >&2
  exit 1
}
command -v psql >/dev/null || {
  echo "psql is required" >&2
  exit 1
}
[[ -f "$SQLITE_DB" ]] || {
  echo "SQLite database not found: $SQLITE_DB" >&2
  exit 1
}

# Hyphenated-UUID-text -> BLOB (NULL / empty-safe).
# CSV NULL '' becomes '', and unhex('') is X'' (not SQL NULL) which breaks FKs.
uuid_blob() {
  local c=$1
  printf "CASE WHEN %s IS NULL OR %s = '' THEN NULL ELSE unhex(replace(%s, '-', '')) END" \
    "$c" "$c" "$c"
}

# timestamptz -> RFC3339 UTC text for sqlx/chrono DateTime<Utc> on SQLite.
# PG ::text yields "2026-10-25 20:42:00.025881+00", which sqlx rejects.
ts_txt() {
  local c=$1
  printf "to_char((%s) AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')" "$c"
}

require_empty_sqlite() {
  local count
  count=$(sqlite3 "$SQLITE_DB" "
    SELECT
      (SELECT COUNT(*) FROM users) +
      (SELECT COUNT(*) FROM sessions) +
      (SELECT COUNT(*) FROM user_passkeys) +
      (SELECT COUNT(*) FROM roots) +
      (SELECT COUNT(*) FROM files) +
      (SELECT COUNT(*) FROM file_metadata) +
      (SELECT COUNT(*) FROM trashed_files) +
      (SELECT COUNT(*) FROM user_roots) +
      (SELECT COUNT(*) FROM root_activity);
  ")
  if [[ "$count" != "0" ]]; then
    echo "Target SQLite database is not empty (${count} rows). Aborting." >&2
    exit 1
  fi
}

export_csv() {
  local out=$1 sql=$2
  psql "$PG_URL" -v ON_ERROR_STOP=1 -c \
    "\\copy (${sql}) TO STDOUT WITH (FORMAT csv, HEADER true, NULL '')" \
    >"$out"
}

WORKDIR=$(mktemp -d)

trap 'rm -rf "$WORKDIR"' EXIT

echo "Checking if target is empty"

require_empty_sqlite

echo "Exporting from Postgres: $WORKDIR"

export_csv "$WORKDIR/users.csv" "
  SELECT id::text, username, password, email, role, otp_secret,
         $(ts_txt created_at), $(ts_txt updated_at)
  FROM users
"

# Skip exporting sessions

export_csv "$WORKDIR/user_passkeys.csv" "
  SELECT id::text, user_id::text, data::text, $(ts_txt created_at)
  FROM user_passkeys
"

export_csv "$WORKDIR/roots.csv" "
  SELECT id::text, name, quota_bytes, owner::text,
         $(ts_txt created_at), $(ts_txt updated_at)
  FROM roots
"

export_csv "$WORKDIR/files.csv" "
  SELECT id::text, root_id::text, created_by::text, parent::text,
         type, name, format, hash, size_bytes, sharing_mode,
         $(ts_txt share_expires_at), $(ts_txt created_at), $(ts_txt updated_at)
  FROM files
"

export_csv "$WORKDIR/file_metadata.csv" "
  SELECT file_id::text, preview_64, preview_256, preview_blob
  FROM file_metadata
"

export_csv "$WORKDIR/trashed_files.csv" "
  SELECT id::text, root_id::text, created_by::text, trashed_by::text,
         parent::text, type, name, format, hash, size_bytes,
         $(ts_txt created_at), $(ts_txt updated_at), $(ts_txt trashed_at)
  FROM trashed_files
"

export_csv "$WORKDIR/user_roots.csv" "
  SELECT root_id::text, user_id::text, access_mode
  FROM user_roots
"

export_csv "$WORKDIR/root_activity.csv" "
  SELECT root_id::text, username, data::text, $(ts_txt created_at)
  FROM root_activity
"

echo "Importing into SQLite"

# Single connection: temp staging + one transaction for deferred self-FKs.
# defer_foreign_keys must be set after BEGIN (autocommit resets it on commit).
# -bail: sqlite3 otherwise exits 0 even when SQL fails.
sqlite3 -bail "$SQLITE_DB" <<SQL
PRAGMA foreign_keys = ON;
BEGIN;
PRAGMA defer_foreign_keys = ON;

CREATE TEMP TABLE _users (
  id TEXT, username TEXT, password TEXT, email TEXT, role TEXT,
  otp_secret TEXT, created_at TEXT, updated_at TEXT
);

.mode csv

.import --skip 1 ${WORKDIR}/users.csv _users

INSERT INTO users (id, username, password, email, role, otp_secret, created_at, updated_at)
SELECT $(uuid_blob id), username, password,
       NULLIF(email, ''), role, NULLIF(otp_secret, ''),
       created_at, updated_at
FROM _users;

DROP TABLE _users;

-- Skip importing sessions

CREATE TEMP TABLE _user_passkeys (
  id TEXT, user_id TEXT, data TEXT, created_at TEXT
);

.import --skip 1 ${WORKDIR}/user_passkeys.csv _user_passkeys

INSERT INTO user_passkeys (id, user_id, data, created_at)
SELECT $(uuid_blob id), $(uuid_blob user_id), data, created_at 
FROM _user_passkeys;

DROP TABLE _user_passkeys;

CREATE TEMP TABLE _roots (
  id TEXT, name TEXT, quota_bytes INTEGER, owner TEXT,
  created_at TEXT, updated_at TEXT
);

.import --skip 1 ${WORKDIR}/roots.csv _roots

INSERT INTO roots (id, name, quota_bytes, owner, created_at, updated_at)
SELECT $(uuid_blob id), name, quota_bytes, $(uuid_blob owner),
       created_at, updated_at
FROM _roots;
DROP TABLE _roots;

CREATE TEMP TABLE _files (
  id TEXT, root_id TEXT, created_by TEXT, parent TEXT, type TEXT, name TEXT,
  format TEXT, hash TEXT, size_bytes INTEGER, sharing_mode TEXT,
  share_expires_at TEXT, created_at TEXT, updated_at TEXT
);

.import --skip 1 ${WORKDIR}/files.csv _files

INSERT INTO files (id, root_id, created_by, parent, "type", name, format, hash, size_bytes, sharing_mode, share_expires_at, created_at, updated_at)
SELECT $(uuid_blob id), $(uuid_blob root_id), $(uuid_blob created_by), $(uuid_blob parent),
       type, name, NULLIF(format, ''), NULLIF(hash, ''), size_bytes,
       NULLIF(sharing_mode, ''), NULLIF(share_expires_at, ''),
       created_at, updated_at
FROM _files;
DROP TABLE _files;

CREATE TEMP TABLE _file_metadata (
  file_id TEXT, preview_64 TEXT, preview_256 TEXT, preview_blob TEXT
);

.import --skip 1 ${WORKDIR}/file_metadata.csv _file_metadata

INSERT INTO file_metadata (file_id, preview_64, preview_256, preview_blob)
SELECT $(uuid_blob file_id),
       NULLIF(preview_64, ''), NULLIF(preview_256, ''), NULLIF(preview_blob, '')
FROM _file_metadata;
DROP TABLE _file_metadata;

CREATE TEMP TABLE _trashed_files (
  id TEXT, root_id TEXT, created_by TEXT, trashed_by TEXT, parent TEXT,
  type TEXT, name TEXT, format TEXT, hash TEXT, size_bytes INTEGER,
  created_at TEXT, updated_at TEXT, trashed_at TEXT
);

.import --skip 1 ${WORKDIR}/trashed_files.csv _trashed_files

INSERT INTO trashed_files (id, root_id, created_by, trashed_by, parent, "type", name, format, hash, size_bytes, created_at, updated_at, trashed_at)
SELECT $(uuid_blob id), $(uuid_blob root_id), $(uuid_blob created_by), $(uuid_blob trashed_by),
       $(uuid_blob parent), type, name, NULLIF(format, ''), NULLIF(hash, ''),
       size_bytes, created_at, updated_at, trashed_at
FROM _trashed_files;
DROP TABLE _trashed_files;

CREATE TEMP TABLE _user_roots (
  root_id TEXT, user_id TEXT, access_mode INTEGER
);

.import --skip 1 ${WORKDIR}/user_roots.csv _user_roots

INSERT INTO user_roots (root_id, user_id, access_mode)
SELECT $(uuid_blob root_id), $(uuid_blob user_id), access_mode
FROM _user_roots;
DROP TABLE _user_roots;

CREATE TEMP TABLE _root_activity (
  root_id TEXT, username TEXT, data TEXT, created_at TEXT
);

.import --skip 1 ${WORKDIR}/root_activity.csv _root_activity

INSERT INTO root_activity (root_id, username, data, created_at)
SELECT $(uuid_blob root_id), username, data, created_at
FROM _root_activity;
DROP TABLE _root_activity;

COMMIT;
SQL

echo "Finished migrating Postgres data to SQLite."
