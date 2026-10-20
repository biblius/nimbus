-- Identifiers and timestamps are generated in Rust so the same repository
-- code can run on Postgres and SQLite.
CREATE TABLE users (
  id UUID PRIMARY KEY NOT NULL,
  username TEXT UNIQUE NOT NULL,
  password TEXT NOT NULL,
  email TEXT,
  role TEXT NOT NULL,
  otp_secret TEXT,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL
);

CREATE UNIQUE INDEX users_email_idx ON users (email)
WHERE
  email IS NOT NULL;

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

CREATE TABLE user_passkeys (
  id BIGSERIAL PRIMARY KEY,
  user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  data JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL
);

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
  -- Null when not publicly shared. `view` allows preview only; `download` also allows downloads.
  -- Sharing a directory sets this (and share_expires_at) on every descendant; unsharing clears them.
  sharing_mode TEXT,
  -- When set, the public share is treated as missing after this instant.
  share_expires_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX files_root_id_parent_idx ON files (root_id, parent);

CREATE INDEX files_root_id_id_idx ON files (root_id, id);

CREATE INDEX files_created_by_idx ON files (created_by);

CREATE INDEX files_parent_idx ON files (parent);

CREATE INDEX files_hash_idx ON files (hash);

CREATE INDEX files_sharing_mode_idx ON files (sharing_mode, share_expires_at)
WHERE
  sharing_mode IS NOT NULL;

-- Preview hashes and other generated artifacts. `file_id` matches `files.id`
-- or `trashed_files.id` No FK because the same id moves between those tables.
CREATE TABLE file_metadata (
  file_id UUID PRIMARY KEY NOT NULL,
  preview_64 TEXT,
  preview_256 TEXT
);

CREATE INDEX file_metadata_preview_64_idx ON file_metadata (preview_64)
WHERE
  preview_64 IS NOT NULL;

CREATE INDEX file_metadata_preview_256_idx ON file_metadata (preview_256)
WHERE
  preview_256 IS NOT NULL;

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
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  trashed_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX trashed_files_root_id_parent_idx ON trashed_files (root_id, parent);

CREATE INDEX trashed_files_root_id_id_idx ON trashed_files (root_id, id);

CREATE INDEX trashed_files_parent_idx ON trashed_files (parent);

CREATE INDEX trashed_files_hash_idx ON trashed_files (hash);

CREATE INDEX trashed_files_trashed_at_idx ON trashed_files (trashed_at);

CREATE TABLE user_folders (
  root_id UUID NOT NULL REFERENCES roots (id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  access_mode BIGINT NOT NULL,
  UNIQUE (root_id, user_id)
);

CREATE INDEX user_folders_user_idx ON user_folders (user_id);
