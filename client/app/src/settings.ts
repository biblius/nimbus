import Database from "@tauri-apps/plugin-sql";
import { parseServerOrigin } from "@nimbus/shared/server-url";

export { parseServerOrigin };

let database: Database | undefined = undefined;
const URL = "sqlite:nimbus.db";

async function _db(): Promise<Database> {
  if (!database) {
    database = await Database.load(URL);
  }
  return database;
}

/** Build the Nimbus API base URL from a server origin. */
export function apiBaseUrl(origin: string): string {
  return `${origin}/api`;
}

export async function loadEndpoint(): Promise<string | null> {
  const db = await _db();

  const rows = await db.select<{ value: string }[]>(
    "SELECT value FROM settings WHERE key = $1",
    ["endpoint"],
  );

  const value = rows[0]?.value?.trim();
  if (!value) {
    return null;
  }

  return parseServerOrigin(value);
}

export async function saveEndpoint(endpoint: string): Promise<void> {
  const origin = parseServerOrigin(endpoint);
  if (!origin) {
    throw new Error("invalid endpoint");
  }

  const db = await _db();

  await db.execute(
    `
    INSERT INTO settings (key, value)
    VALUES ($1, $2) 
    ON CONFLICT(key) DO UPDATE 
    SET value = excluded.value
    `,
    ["endpoint", origin],
  );
}

/** Load the single persisted Bearer session token, if any. */
export async function loadAccessToken(): Promise<string | null> {
  const db = await _db();

  const rows = await db.select<{ token: string }[]>(
    "SELECT token FROM access_token WHERE id = 1",
  );

  const token = rows[0]?.token?.trim();
  return token ? token : null;
}

/** Replace the single access token (insert or update the sole row). */
export async function saveAccessToken(token: string): Promise<void> {
  const db = await _db();

  await db.execute(
    `
    INSERT INTO access_token (id, token)
    VALUES (1, $1)
    ON CONFLICT(id) DO UPDATE
    SET token = excluded.token
    `,
    [token],
  );
}

/** Clear the persisted access token. */
export async function clearAccessToken(): Promise<void> {
  const db = await _db();
  await db.execute("DELETE FROM access_token WHERE id = 1");
}
