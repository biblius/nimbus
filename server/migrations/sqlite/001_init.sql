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
  preview TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX files_root_id_idx ON files (root_id);

CREATE INDEX files_created_by_idx ON files (created_by);

CREATE INDEX files_parent_idx ON files (parent);

CREATE INDEX files_hash_idx ON files (hash);

CREATE INDEX files_preview_idx ON files (preview)
WHERE
  preview IS NOT NULL;

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
  preview TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  trashed_at TEXT NOT NULL
);

CREATE INDEX trashed_files_root_id_idx ON trashed_files (root_id);

CREATE INDEX trashed_files_trashed_by_idx ON trashed_files (trashed_by);

CREATE INDEX trashed_files_parent_idx ON trashed_files (parent);

CREATE INDEX trashed_files_hash_idx ON trashed_files (hash);

CREATE INDEX trashed_files_preview_idx ON trashed_files (preview)
WHERE
  preview IS NOT NULL;

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

-- Deletion worker queue.
CREATE TABLE file_deletion_queue (
  id BLOB NOT NULL,
  file_id BLOB NOT NULL,
  origin TEXT NOT NULL,
  created_by TEXT NOT NULL,
  created_at TEXT NOT NULL,
  error TEXT
);

CREATE UNIQUE INDEX file_deletion_queue_file_idx ON file_deletion_queue (origin, file_id);

CREATE INDEX file_deletion_queue_request_idx ON file_deletion_queue (id)
WHERE
  error IS NULL;

CREATE INDEX file_deletion_queue_pending_idx ON file_deletion_queue (created_by)
WHERE
  error IS NULL;
