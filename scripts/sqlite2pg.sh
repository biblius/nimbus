#!/usr/bin/env bash
# Export Nimbus data from SQLite and import into Postgres (CSV + type remaps).
#
# Usage:
#   ./scripts/sqlite2pg.sh <sqlite-db-path> <postgres-url>
#
# Prerequisites:
#   - Target Postgres database already has the Nimbus schema
#     (sqlx migrate run --source server/migrations/postgres).
#   - Target tables must be empty.
#   - Copy UPLOAD_DIR (file BLOBs) separately; this script moves DB rows only.
#
# Example:
#   ./scripts/sqlite2pg.sh ./data/nimbus.db \
#     'postgres://nimbus:nimbus@127.0.0.1:5434/nimbus'
set -euo pipefail

usage() {
  sed -n '2,16p' "$0" | sed 's/^# \?//'
  exit 1
}

[[ $# -eq 2 ]] || usage

SQLITE_DB=$1
PG_URL=$2

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

# UUID BLOB -> hyphenated text (NULL-safe).
uuid_txt() {
  local c=$1
  printf "CASE WHEN %s IS NULL THEN NULL ELSE lower(substr(hex(%s),1,8)||'-'||substr(hex(%s),9,4)||'-'||substr(hex(%s),13,4)||'-'||substr(hex(%s),17,4)||'-'||substr(hex(%s),21,12)) END" \
    "$c" "$c" "$c" "$c" "$c" "$c"
}

require_empty_pg() {
  local count
  count=$(psql "$PG_URL" -Atqc "
    SELECT COALESCE(SUM(n), 0) FROM (
      SELECT COUNT(*) AS n FROM users
      UNION ALL SELECT COUNT(*) FROM sessions
      UNION ALL SELECT COUNT(*) FROM user_passkeys
      UNION ALL SELECT COUNT(*) FROM roots
      UNION ALL SELECT COUNT(*) FROM files
      UNION ALL SELECT COUNT(*) FROM file_metadata
      UNION ALL SELECT COUNT(*) FROM trashed_files
      UNION ALL SELECT COUNT(*) FROM user_roots
      UNION ALL SELECT COUNT(*) FROM root_activity
    ) t;
  ")
  if [[ "$count" != "0" ]]; then
    echo "Target Postgres database is not empty (${count} rows). Aborting." >&2
    exit 1
  fi
}

export_csv() {
  local out=$1 sql=$2
  sqlite3 -header -csv "$SQLITE_DB" "$sql" >"$out"
}

WORKDIR=$(mktemp -d)

trap 'rm -rf "$WORKDIR"' EXIT

echo "Checking target is empty"

require_empty_pg

echo "Exporting from SQLite to $WORKDIR"

export_csv "$WORKDIR/users.csv" "
SELECT
  $(uuid_txt id) AS id,
  username,
  password,
  email,
  role,
  otp_secret,
  created_at,
  updated_at
FROM users;
"

export_csv "$WORKDIR/user_passkeys.csv" "
SELECT
  $(uuid_txt id) AS id,
  $(uuid_txt user_id) AS user_id,
  data,
  created_at
FROM user_passkeys;
"

export_csv "$WORKDIR/roots.csv" "
SELECT
  $(uuid_txt id) AS id,
  name,
  quota_bytes,
  $(uuid_txt owner) AS owner,
  created_at,
  updated_at
FROM roots;
"

export_csv "$WORKDIR/files.csv" "
SELECT
  $(uuid_txt id) AS id,
  $(uuid_txt root_id) AS root_id,
  $(uuid_txt created_by) AS created_by,
  $(uuid_txt parent) AS parent,
  \"type\",
  name,
  format,
  hash,
  size_bytes,
  sharing_mode,
  share_expires_at,
  created_at,
  updated_at
FROM files;
"

export_csv "$WORKDIR/file_metadata.csv" "
SELECT
  $(uuid_txt file_id) AS file_id,
  preview_64,
  preview_256,
  preview_blob
FROM file_metadata;
"

export_csv "$WORKDIR/trashed_files.csv" "
SELECT
  $(uuid_txt id) AS id,
  $(uuid_txt root_id) AS root_id,
  $(uuid_txt created_by) AS created_by,
  $(uuid_txt trashed_by) AS trashed_by,
  $(uuid_txt parent) AS parent,
  \"type\",
  name,
  format,
  hash,
  size_bytes,
  created_at,
  updated_at,
  trashed_at
FROM trashed_files;
"

export_csv "$WORKDIR/user_roots.csv" "
SELECT
  $(uuid_txt root_id) AS root_id,
  $(uuid_txt user_id) AS user_id,
  access_mode
FROM user_roots;
"

export_csv "$WORKDIR/root_activity.csv" "
SELECT
  $(uuid_txt root_id) AS root_id,
  username,
  data,
  created_at
FROM root_activity;
"

echo "Importing into Postgres"

# One transaction so deferred FKs (files.parent, trashed_files.parent) hold.
psql "$PG_URL" -v ON_ERROR_STOP=1 <<SQL
BEGIN;
SET CONSTRAINTS ALL DEFERRED;

\\copy users(id, username, password, email, role, otp_secret, created_at, updated_at)
FROM '$WORKDIR/users.csv' WITH (FORMAT csv, HEADER true, NULL '')

\\copy user_passkeys(id, user_id, data, created_at)
FROM '$WORKDIR/user_passkeys.csv' WITH (FORMAT csv, HEADER true, NULL '')

\\copy roots(id, name, quota_bytes, owner, created_at, updated_at)
FROM '$WORKDIR/roots.csv' WITH (FORMAT csv, HEADER true, NULL '')

\\copy files(id, root_id, created_by, parent, "type", name, format, hash, size_bytes, sharing_mode, share_expires_at, created_at, updated_at) 
FROM '$WORKDIR/files.csv' WITH (FORMAT csv, HEADER true, NULL '')

\\copy file_metadata(file_id, preview_64, preview_256, preview_blob) 
FROM '$WORKDIR/file_metadata.csv' WITH (FORMAT csv, HEADER true, NULL '')

\\copy trashed_files(id, root_id, created_by, trashed_by, parent, "type", name, format, hash, size_bytes, created_at, updated_at, trashed_at) 
FROM '$WORKDIR/trashed_files.csv' WITH (FORMAT csv, HEADER true, NULL '')

\\copy user_roots(root_id, user_id, access_mode) 
FROM '$WORKDIR/user_roots.csv' WITH (FORMAT csv, HEADER true, NULL '')

\\copy root_activity(root_id, username, data, created_at) 
FROM '$WORKDIR/root_activity.csv' WITH (FORMAT csv, HEADER true, NULL '')

COMMIT;
SQL

echo "Successfully migrated SQLite to postgres"
