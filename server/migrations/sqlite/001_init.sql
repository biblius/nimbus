-- Identifiers and timestamps are generated in Rust.
-- sqlx stores Uuid as a 16-byte BLOB and DateTime<Utc> as TEXT.
CREATE TABLE users (
  id BLOB PRIMARY KEY NOT NULL,
  username TEXT UNIQUE NOT NULL,
  password TEXT NOT NULL,
  email TEXT,
  role TEXT NOT NULL CHECK (role IN ('admin', 'user')),
  otp_secret TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE UNIQUE INDEX users_email_idx ON users (email)
WHERE
  email IS NOT NULL;

-- A root is the quota and access boundary. It is not a file row: directories and
-- files live in `files` and point back here with `root_id`.
CREATE TABLE roots (
  id BLOB PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  quota_bytes INTEGER NOT NULL DEFAULT 0,
  "primary" BOOLEAN NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE files (
  id BLOB PRIMARY KEY NOT NULL,
  root_id BLOB NOT NULL REFERENCES roots (id),
  created_by BLOB NOT NULL REFERENCES users (id),
  parent BLOB REFERENCES files (id),
  "type" TEXT NOT NULL,
  name TEXT NOT NULL,
  family TEXT,
  format TEXT,
  hash TEXT,
  size_bytes INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE INDEX files_root_id_idx ON files (root_id);

CREATE INDEX files_created_by_idx ON files (created_by);

CREATE INDEX files_parent_idx ON files (parent);

CREATE INDEX files_hash_idx ON files (hash);

CREATE UNIQUE INDEX files_root_name_idx ON files (root_id, name)
WHERE
  deleted_at IS NULL
  AND parent IS NULL;

CREATE UNIQUE INDEX files_sibling_name_idx ON files (parent, name)
WHERE
  deleted_at IS NULL
  AND parent IS NOT NULL;

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

-- A row grants access to a root. "primary" is the user's private space.
-- A user has one primary root.
CREATE TABLE user_folders (
  root_id BLOB NOT NULL REFERENCES roots (id) ON DELETE CASCADE,
  user_id BLOB NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  UNIQUE (root_id, user_id)
);

CREATE UNIQUE INDEX user_folders_one_primary ON user_folders (user_id)
WHERE
  "primary" = 1;

CREATE INDEX user_folders_user_idx ON user_folders (user_id);
