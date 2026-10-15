-- Postgres seed. Safe to run again: existing rows are left in place.
--
-- Admin: username `admin`, password `admin`.
-- 20 users, seed-01 through seed-20. Password is `password`.
-- Five shared roots with overlapping memberships (see below).
--
-- psql "$DATABASE_URL" -f server/seeds/dev.sql

-- Password hashes (argon2id, salt `seedsalt`):
--   admin    -> admin
--   seed-*   -> password
INSERT INTO
  users (
    id,
    username,
    password,
    email,
    role,
    created_at,
    updated_at
  )
VALUES
  (
    '00000000-0000-4000-8000-000000000000',
    'admin',
    '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$vh6ncdxQbKo/CUhSZy3PXcwHEt8SN1KuCmDVFruJTxo',
    NULL,
    'admin',
    timestamptz '2026-01-01 00:00:00+00',
    timestamptz '2026-01-01 00:00:00+00'
  ) ON CONFLICT (username) DO NOTHING;

INSERT INTO
  users (
    id,
    username,
    password,
    email,
    role,
    created_at,
    updated_at
  )
SELECT
  (
    '00000000-0000-4000-8000-' || lpad(to_hex(n), 12, '0')
  )::uuid,
  'seed-' || lpad(n::text, 2, '0'),
  '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw',
  'seed-' || lpad(n::text, 2, '0') || '@example.com',
  'user',
  timestamptz '2026-01-01 00:00:00+00',
  timestamptz '2026-01-01 00:00:00+00' + make_interval(mins => n)
FROM
  generate_series(1, 20) AS n ON CONFLICT (username) DO NOTHING;

INSERT INTO
  roots (
    id,
    name,
    quota_bytes,
    owner,
    created_at,
    updated_at
  )
VALUES
  -- Admin private root
  (
    '00000000-0000-4000-8000-0000000000c1',
    'admin',
    0,
    '00000000-0000-4000-8000-000000000000',
    timestamptz '2026-01-01 00:00:00+00',
    timestamptz '2026-01-01 00:00:00+00'
  ),
  -- Shared roots (owner IS NULL)
  (
    '00000000-0000-4000-8000-0000000000c0',
    'Seed shared',
    0,
    NULL,
    timestamptz '2026-01-01 00:00:00+00',
    timestamptz '2026-01-01 00:00:00+00'
  ),
  (
    '00000000-0000-4000-8000-0000000000c2',
    'Team Alpha',
    0,
    NULL,
    timestamptz '2026-01-01 00:00:00+00',
    timestamptz '2026-01-01 00:00:00+00'
  ),
  (
    '00000000-0000-4000-8000-0000000000c3',
    'Team Beta',
    0,
    NULL,
    timestamptz '2026-01-01 00:00:00+00',
    timestamptz '2026-01-01 00:00:00+00'
  ),
  (
    '00000000-0000-4000-8000-0000000000c4',
    'Design',
    0,
    NULL,
    timestamptz '2026-01-01 00:00:00+00',
    timestamptz '2026-01-01 00:00:00+00'
  ),
  (
    '00000000-0000-4000-8000-0000000000c5',
    'All hands',
    0,
    NULL,
    timestamptz '2026-01-01 00:00:00+00',
    timestamptz '2026-01-01 00:00:00+00'
  ) ON CONFLICT (id) DO NOTHING;

-- Seed shared: seed-01 .. seed-15
INSERT INTO
  user_folders (root_id, user_id)
SELECT
  '00000000-0000-4000-8000-0000000000c0',
  (
    '00000000-0000-4000-8000-' || lpad(to_hex(n), 12, '0')
  )::uuid
FROM
  generate_series(1, 15) AS n ON CONFLICT (root_id, user_id) DO NOTHING;

-- Team Alpha: seed-01 .. seed-05
INSERT INTO
  user_folders (root_id, user_id)
SELECT
  '00000000-0000-4000-8000-0000000000c2',
  (
    '00000000-0000-4000-8000-' || lpad(to_hex(n), 12, '0')
  )::uuid
FROM
  generate_series(1, 5) AS n ON CONFLICT (root_id, user_id) DO NOTHING;

-- Team Beta: seed-04 .. seed-10 (overlaps Alpha)
INSERT INTO
  user_folders (root_id, user_id)
SELECT
  '00000000-0000-4000-8000-0000000000c3',
  (
    '00000000-0000-4000-8000-' || lpad(to_hex(n), 12, '0')
  )::uuid
FROM
  generate_series(4, 10) AS n ON CONFLICT (root_id, user_id) DO NOTHING;

-- Design: seed-11 .. seed-15
INSERT INTO
  user_folders (root_id, user_id)
SELECT
  '00000000-0000-4000-8000-0000000000c4',
  (
    '00000000-0000-4000-8000-' || lpad(to_hex(n), 12, '0')
  )::uuid
FROM
  generate_series(11, 15) AS n ON CONFLICT (root_id, user_id) DO NOTHING;

-- All hands: seed-01 .. seed-20
INSERT INTO
  user_folders (root_id, user_id)
SELECT
  '00000000-0000-4000-8000-0000000000c5',
  (
    '00000000-0000-4000-8000-' || lpad(to_hex(n), 12, '0')
  )::uuid
FROM
  generate_series(1, 20) AS n ON CONFLICT (root_id, user_id) DO NOTHING;
