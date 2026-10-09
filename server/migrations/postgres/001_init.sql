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

-- A root is the quota and access boundary. It is not a file row: directories and
-- files live in `files` and point back here with `root_id`.
CREATE TABLE roots (
  id UUID PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  quota_bytes BIGINT NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE files (
  id UUID PRIMARY KEY NOT NULL,
  root_id UUID NOT NULL REFERENCES roots (id),
  created_by UUID NOT NULL REFERENCES users (id),
  parent UUID REFERENCES files (id),
  "type" TEXT NOT NULL,
  name TEXT NOT NULL,
  family TEXT,
  format TEXT,
  hash TEXT,
  size_bytes BIGINT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  deleted_at TIMESTAMPTZ
);

CREATE INDEX files_root_id_idx ON files (root_id);

CREATE INDEX files_created_by_idx ON files (created_by);

CREATE INDEX files_parent_idx ON files (parent);

CREATE INDEX files_hash_idx ON files (hash);

-- Live names are unique among top-level entries of a root, and among siblings.
CREATE UNIQUE INDEX files_root_name_idx ON files (root_id, name)
WHERE
  deleted_at IS NULL
  AND parent IS NULL;

CREATE UNIQUE INDEX files_sibling_name_idx ON files (parent, name)
WHERE
  deleted_at IS NULL
  AND parent IS NOT NULL;

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

-- A row grants access to a root. 
-- A user has one primary root.
CREATE TABLE user_folders (
  root_id UUID NOT NULL REFERENCES roots (id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  "primary" BOOLEAN NOT NULL,
  UNIQUE (root_id, user_id)
);

CREATE UNIQUE INDEX user_folders_one_primary ON user_folders (user_id)
WHERE
  "primary";

CREATE INDEX user_folders_user_idx ON user_folders (user_id);
