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
-- `owner` is set for a user's private root and null for a shared root.
-- A user has at most one private root.
CREATE TABLE roots (
  id UUID PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  quota_bytes BIGINT NOT NULL DEFAULT 0,
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
  created_by UUID NOT NULL REFERENCES users (id),
  parent UUID REFERENCES files (id),
  "type" TEXT NOT NULL,
  name TEXT NOT NULL,
  format TEXT,
  hash TEXT,
  size_bytes BIGINT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX files_root_id_idx ON files (root_id);

CREATE INDEX files_created_by_idx ON files (created_by);

CREATE INDEX files_parent_idx ON files (parent);

CREATE INDEX files_hash_idx ON files (hash);

-- Rows leave `files` when trashed. A directly trashed row has `parent` NULL and
-- keeps its previous parent in `original_parent`. Descendants keep their parent.
CREATE TABLE trashed_files (
  id UUID PRIMARY KEY NOT NULL,
  root_id UUID NOT NULL REFERENCES roots (id),
  trashed_by UUID NOT NULL REFERENCES users (id),
  parent UUID REFERENCES trashed_files (id) DEFERRABLE INITIALLY DEFERRED,
  original_parent UUID,
  "type" TEXT NOT NULL,
  name TEXT NOT NULL,
  format TEXT,
  hash TEXT,
  size_bytes BIGINT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX trashed_files_root_id_idx ON trashed_files (root_id);

CREATE INDEX trashed_files_trashed_by_idx ON trashed_files (trashed_by);

CREATE INDEX trashed_files_parent_idx ON trashed_files (parent);

CREATE INDEX trashed_files_hash_idx ON trashed_files (hash);

CREATE INDEX trashed_files_created_at_idx ON trashed_files (created_at);

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
