-- Identifiers and timestamps are generated in Rust.
-- sqlx stores Uuid as a 16-byte BLOB and DateTime<Utc> as TEXT.
CREATE TABLE users (
  id BLOB PRIMARY KEY NOT NULL,
  username TEXT UNIQUE NOT NULL,
  password TEXT NOT NULL,
  email TEXT,
  role TEXT NOT NULL,
  otp_secret TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX users_email_idx ON users (email)
WHERE
  email IS NOT NULL;

CREATE TABLE user_passkeys (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id BLOB NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  data TEXT NOT NULL,
  created_at TEXT NOT NULL
);

-- A root is the quota and access boundary. It is not a file row: directories and
-- files live in `files` and point back here with `root_id`.
-- `owner` is set for a user's private root and null for a shared root.
-- A user has at most one private root.
CREATE TABLE roots (
  id BLOB PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  quota_bytes INTEGER NOT NULL DEFAULT 0,
  owner BLOB REFERENCES users (id),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX roots_owner_idx ON roots (owner)
WHERE
  owner IS NOT NULL;

CREATE TABLE files (
  id BLOB PRIMARY KEY NOT NULL,
  root_id BLOB NOT NULL REFERENCES roots (id),
  created_by BLOB REFERENCES users (id) ON DELETE SET NULL,
  parent BLOB REFERENCES files (id) ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED,
  "type" TEXT NOT NULL,
  name TEXT NOT NULL,
  format TEXT,
  hash TEXT,
  size_bytes INTEGER NOT NULL,
  -- Null when not publicly shared. `view` allows preview only; `download` also allows downloads.
  -- Sharing a directory sets this (and share_expires_at) on every descendant; unsharing clears them.
  sharing_mode TEXT,
  -- When set, the public share is treated as missing after this instant.
  share_expires_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX files_root_id_parent_idx ON files (root_id, parent);

CREATE INDEX files_root_id_id_idx ON files (root_id, id);

CREATE INDEX files_created_by_idx ON files (created_by);

CREATE INDEX files_parent_idx ON files (parent);

CREATE INDEX files_hash_idx ON files (hash);

CREATE INDEX files_sharing_mode_idx ON files (sharing_mode, share_expires_at)
WHERE
  sharing_mode IS NOT NULL;

-- Preview hashes. `file_id` matches `files.id` or `trashed_files.id` (no FK).
CREATE TABLE file_metadata (
  file_id BLOB PRIMARY KEY NOT NULL,
  preview_64 TEXT,
  preview_256 TEXT
);

CREATE INDEX file_metadata_preview_64_idx ON file_metadata (preview_64)
WHERE
  preview_64 IS NOT NULL;

CREATE INDEX file_metadata_preview_256_idx ON file_metadata (preview_256)
WHERE
  preview_256 IS NOT NULL;

-- Rows leave `files` when trashed. A directly trashed row has `parent` NULL.
-- Descendants keep their parent.
CREATE TABLE trashed_files (
  id BLOB PRIMARY KEY NOT NULL,
  root_id BLOB NOT NULL REFERENCES roots (id),
  created_by BLOB REFERENCES users (id) ON DELETE SET NULL,
  trashed_by BLOB REFERENCES users (id) ON DELETE SET NULL,
  parent BLOB REFERENCES trashed_files (id) ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED,
  "type" TEXT NOT NULL,
  name TEXT NOT NULL,
  format TEXT,
  hash TEXT,
  size_bytes INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  trashed_at TEXT NOT NULL
);

CREATE INDEX trashed_files_root_id_parent_idx ON trashed_files (root_id, parent);

CREATE INDEX trashed_files_root_id_id_idx ON trashed_files (root_id, id);

CREATE INDEX trashed_files_trashed_by_idx ON trashed_files (trashed_by);

CREATE INDEX trashed_files_parent_idx ON trashed_files (parent);

CREATE INDEX trashed_files_hash_idx ON trashed_files (hash);

CREATE INDEX trashed_files_trashed_at_idx ON trashed_files (trashed_at);

CREATE TABLE sessions (
  id BLOB PRIMARY KEY NOT NULL,
  user_id BLOB NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  remote_address TEXT,
  user_agent TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  expires_at TEXT
);

CREATE INDEX sessions_user_id_idx ON sessions (user_id);

-- Membership of a shared root. Private roots are not recorded here.
CREATE TABLE user_folders (
  root_id BLOB NOT NULL REFERENCES roots (id) ON DELETE CASCADE,
  user_id BLOB NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  UNIQUE (root_id, user_id)
);

CREATE INDEX user_folders_user_idx ON user_folders (user_id);
