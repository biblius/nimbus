import Database from "@tauri-apps/plugin-sql";

let database: Database | undefined = undefined;
const URL = "sqlite:nimbus.db";

async function _db(): Promise<Database> {
  if (!database) {
    database = await Database.load(URL);
  }
  return database;
}

/** Trim, strip a trailing slash, and strip a trailing `/api` if present. */
export function normalizeOrigin(url: string): string {
  let origin = url.trim().replace(/\/$/, "");
  if (origin.endsWith("/api")) {
    origin = origin.slice(0, -"/api".length).replace(/\/$/, "");
  }
  return origin;
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

  return value ? value : null;
}

export async function saveEndpoint(endpoint: string): Promise<void> {
  const db = await _db();

  await db.execute(
    `
    INSERT INTO settings (key, value)
    VALUES ($1, $2) 
    ON CONFLICT(key) DO UPDATE 
    SET value = excluded.value
    `,
    ["endpoint", endpoint],
  );
}
