# Migrate SQLite <-> Postgres

Migration scripts:

- SQLite -> Postgres: [`scripts/sqlite2pg.sh`](https://github.com/biblius/nimbus/blob/master/scripts/sqlite2pg.sh)
- Postgres -> SQLite: [`scripts/pg2sqlite.sh`](https://github.com/biblius/nimbus/blob/master/scripts/pg2sqlite.sh)

This is for if you ever find yourself in a situation where you want to
migrate Nimbus from an SQLite database to a Postgres one (or vice versa).

## What you need

- `sqlite3` and `psql` on wherever your original database is
- A fresh (completely empty) migrated database (see below)
- Matching Nimbus release / migration versions

## Migration

Step 0 is to have an empty and migrated database. How you do this is up
to you. Your sane options are:

- Run the Nimbus binary with the new database driver and let it migrate
  the DB for you, or
- Do it with `sqlx`:
  ```bash
  cargo install sqlx-cli # if you don't already have it
  DATABASE_URL="<YOUR_DATABASE_URL>" sqlx migrate run --source <NIMBUS_MIGRATIONS>
  ```

Nimbus defines its data types like the following:

| Postgres      | SQLite            |
| ------------- | ----------------- |
| `UUID`        | `BLOB` (16 bytes) |
| `TIMESTAMPTZ` | `TEXT` (ISO-8601) |
| `JSONB`       | `TEXT`            |
| `BIGINT`      | `INTEGER`         |

A raw `sqlite3 .dump | psql`, and vice versa, fails because these 2 SQL engines do not support the same data types.
To mitigate this we use CSV exports and imports.

The scripts export every table (excluding sessions) on the input (1st arg)
database to a CSV, converting any incompatible types in the process,
and import them to whatever output (2nd arg) points to.

In this example we'll migrate SQLite to Postgres, but the process is the same
for SQLite, just make sure to use the correct script and set the source
and target accordingly. We'll use the default DBs as when developing.

1. Set the source and target databases

```bash
export NIMBUS_SOURCE_DB=./data/nimbus.db
export NIMBUS_TARGET_DB=postgres://nimbus:nimbus@127.0.0.1:5434/nimbus
```

2. Run the migration script:

```bash
./scripts/sqlite2pg.sh "$NIMBUS_SOURCE_DB" "$NIMBUS_TARGET_DB"
```

3. ???

4. Profit, but just in case, check and compare the counts.

```bash
psql "$NIMBUS_SOURCE_DB" -c "SELECT
  (SELECT COUNT(*) FROM users) AS users,
  (SELECT COUNT(*) FROM files) AS files,
  (SELECT COUNT(*) FROM roots) AS roots;"

sqlite3 "$NIMBUS_TARGET_DB" "SELECT
  (SELECT COUNT(*) FROM users) AS users,
  (SELECT COUNT(*) FROM files) AS files,
  (SELECT COUNT(*) FROM roots) AS roots;"
```
