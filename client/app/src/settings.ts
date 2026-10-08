import Database from "@tauri-apps/plugin-sql";

const CONNECTION = "sqlite:nimbus.db";

export async function loadEndpoint(): Promise<string> {
  const db = await Database.load(CONNECTION);
  const rows = await db.select<{ value: string }[]>(
    "SELECT value FROM settings WHERE key = $1",
    ["endpoint"],
  );
  return rows[0]?.value ?? "http://localhost:42069";
}

export async function saveEndpoint(endpoint: string): Promise<void> {
  const db = await Database.load(CONNECTION);
  await db.execute(
    "INSERT INTO settings (key, value) VALUES ($1, $2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    ["endpoint", endpoint],
  );
}
