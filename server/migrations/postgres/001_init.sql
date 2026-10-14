-- Identifiers and timestamps are generated in Rust so the same repository
-- code can run on Postgres and SQLite.
CREATE TABLE users (
  id UUID PRIMARY KEY NOT NULL,
  username TEXT UNIQUE NOT NULL,
  password TEXT,
  email TEXT,
  role TEXT NOT NULL,
  otp_secret TEXT,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL
);

CREATE UNIQUE INDEX users_email_idx ON users (email)
WHERE
  email IS NOT NULL;

-- A root directory, either private or shared.
-- `owner` is set for a user's private root and null for a shared root.
-- A user can have only one private root.
CREATE TABLE roots (
  id UUID PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  -- A quota of 0 means unlimited.
  quota_bytes BIGINT NOT NULL DEFAULT 0,
  -- If present, the root is private.
  owner UUID REFERENCES users (id),
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL
);

CREATE UNIQUE INDEX roots_owner_idx ON roots (owner)
WHERE
  owner IS NOT NULL;

CREATE TABLE files (
  id UUID PRIMARY KEY NOT NULL,
  root_id UUID NOT NULL REFERENCES roots (id),
  created_by UUID REFERENCES users (id) ON DELETE SET NULL,
  -- We allows this to be set to null on parent deletion because
  -- it simplifies hard deletion logic. 
  -- The only time this can change is when a file is trashed directly
  -- (in which case it becomes null), or when moved.
  -- The only time this can get set to null is during hard deletion, i.e.
  -- when either users hard delete files manually or when the trash expiration
  -- worker picks it up.
  parent UUID REFERENCES files (id) ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED,
  "type" TEXT NOT NULL,
  name TEXT NOT NULL,
  -- Always null for directories
  format TEXT,
  -- Always null for directories
  hash TEXT,
  -- Always null for directories
  size_bytes BIGINT NOT NULL,
  -- Content hash of the generated preview blob; null until processing finishes (or for non-previewable files)
  preview TEXT,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX files_root_id_idx ON files (root_id);

CREATE INDEX files_created_by_idx ON files (created_by);

CREATE INDEX files_parent_idx ON files (parent);

CREATE INDEX files_hash_idx ON files (hash);

CREATE INDEX files_preview_idx ON files (preview)
WHERE
  preview IS NOT NULL;

-- Used for soft deletion. Files end up here from `files`.
-- A file that is directly deleted from `files` has its parent set to null
-- and always recursively pulls with it all its children.
-- Its children keep the parent pointing to the file to keep the relation.
-- The same reasoning for ON DELETE SET NULL applies as that of `files`.
CREATE TABLE trashed_files (
  id UUID PRIMARY KEY NOT NULL,
  root_id UUID NOT NULL REFERENCES roots (id),
  created_by UUID REFERENCES users (id) ON DELETE SET NULL,
  trashed_by UUID REFERENCES users (id) ON DELETE SET NULL,
  parent UUID REFERENCES trashed_files (id) ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED,
  "type" TEXT NOT NULL,
  name TEXT NOT NULL,
  format TEXT,
  hash TEXT,
  size_bytes BIGINT NOT NULL,
  preview TEXT,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  trashed_at TIMESTAMPTZ NOT NULL
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
  id UUID PRIMARY KEY NOT NULL,
  user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  remote_address TEXT,
  user_agent TEXT,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  expires_at TIMESTAMPTZ
);

CREATE INDEX sessions_user_id_idx ON sessions (user_id);

-- Membership of a shared root. Private roots are not recorded here.
CREATE TABLE user_folders (
  root_id UUID NOT NULL REFERENCES roots (id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  UNIQUE (root_id, user_id)
);

CREATE INDEX user_folders_user_idx ON user_folders (user_id);

-- Deletion worker queue.
CREATE TABLE file_deletion_queue (
  -- Used for live updates, see queue worker.
  id UUID NOT NULL,
  -- Points to either `files` or `trashed_files` depending on the `origin` column. ID uniqueness is guaranteed across both tables, since the only way
  -- files can end up in the trash is if they existed in files,
  -- but there are split seconds where the same file from trash and live
  -- can coexist here.
  file_id UUID NOT NULL,
  origin TEXT NOT NULL,
  -- `A user id, or SYSTEM when the expiry worker queued the row.
  created_by TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  -- A row with `error` set indicates a failure to delete the file and is not retried unless the error is set to NULL.
  error TEXT
);

CREATE UNIQUE INDEX file_deletion_queue_file_idx ON file_deletion_queue (origin, file_id);

CREATE INDEX file_deletion_queue_request_idx ON file_deletion_queue (id)
WHERE
  error IS NULL;

CREATE INDEX file_deletion_queue_pending_idx ON file_deletion_queue (created_by)
WHERE
  error IS NULL;
