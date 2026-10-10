-- Postgres seed. Safe to run again: existing rows are left in place.
--
-- 20 users, seed-01 through seed-20. Password is `password`.
-- Shared folder "Seed shared" includes seed-01 through seed-15.
--
-- psql "$DATABASE_URL" -f server/seeds/dev.sql

INSERT INTO users (id, username, password, email, role, created_at, updated_at)
VALUES
  ('00000000-0000-4000-8000-000000000001', 'seed-01', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-01@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:01:00+00'),
  ('00000000-0000-4000-8000-000000000002', 'seed-02', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-02@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:02:00+00'),
  ('00000000-0000-4000-8000-000000000003', 'seed-03', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-03@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:03:00+00'),
  ('00000000-0000-4000-8000-000000000004', 'seed-04', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-04@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:04:00+00'),
  ('00000000-0000-4000-8000-000000000005', 'seed-05', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-05@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:05:00+00'),
  ('00000000-0000-4000-8000-000000000006', 'seed-06', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-06@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:06:00+00'),
  ('00000000-0000-4000-8000-000000000007', 'seed-07', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-07@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:07:00+00'),
  ('00000000-0000-4000-8000-000000000008', 'seed-08', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-08@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:08:00+00'),
  ('00000000-0000-4000-8000-000000000009', 'seed-09', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-09@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:09:00+00'),
  ('00000000-0000-4000-8000-00000000000a', 'seed-10', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-10@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:10:00+00'),
  ('00000000-0000-4000-8000-00000000000b', 'seed-11', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-11@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:11:00+00'),
  ('00000000-0000-4000-8000-00000000000c', 'seed-12', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-12@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:12:00+00'),
  ('00000000-0000-4000-8000-00000000000d', 'seed-13', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-13@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:13:00+00'),
  ('00000000-0000-4000-8000-00000000000e', 'seed-14', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-14@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:14:00+00'),
  ('00000000-0000-4000-8000-00000000000f', 'seed-15', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-15@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:15:00+00'),
  ('00000000-0000-4000-8000-000000000010', 'seed-16', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-16@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:16:00+00'),
  ('00000000-0000-4000-8000-000000000011', 'seed-17', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-17@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:17:00+00'),
  ('00000000-0000-4000-8000-000000000012', 'seed-18', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-18@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:18:00+00'),
  ('00000000-0000-4000-8000-000000000013', 'seed-19', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-19@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:19:00+00'),
  ('00000000-0000-4000-8000-000000000014', 'seed-20', '$argon2id$v=19$m=19456,t=2,p=1$c2VlZHNhbHQ$GDjk0RZ9fyosjqd5ot0J/5U6ZtzA9JYcSEhuDL/8gXw', 'seed-20@example.com', 'user', timestamptz '2026-01-01 00:00:00+00', timestamptz '2026-01-01 00:20:00+00')
ON CONFLICT (username) DO NOTHING;

INSERT INTO roots (id, name, quota_bytes, owner, created_at, updated_at)
VALUES (
  '00000000-0000-4000-8000-0000000000c0',
  'Seed shared',
  0,
  NULL,
  timestamptz '2026-01-01 00:00:00+00',
  timestamptz '2026-01-01 00:00:00+00'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO user_folders (root_id, user_id)
SELECT '00000000-0000-4000-8000-0000000000c0', id
FROM users
WHERE username IN (
  'seed-01',
  'seed-02',
  'seed-03',
  'seed-04',
  'seed-05',
  'seed-06',
  'seed-07',
  'seed-08',
  'seed-09',
  'seed-10',
  'seed-11',
  'seed-12',
  'seed-13',
  'seed-14',
  'seed-15'
)
ON CONFLICT (root_id, user_id) DO NOTHING;
