/**
 * Thin localStorage key/value helpers. Failures (private mode, quota) are swallowed
 * so callers can treat storage as best-effort.
 */

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // ignore
  }
}

function remove(key: string): void {
  try {
    localStorage.removeItem(key);
  } catch {
    // ignore
  }
}

export type KvEntry<T> = {
  key: string;
  get: () => T;
  set: (value: T) => void;
  clear: () => void;
};

function stringEntry(key: string, fallback: string): KvEntry<string> {
  return {
    key,
    get: () => read(key) ?? fallback,
    set: (value) => write(key, value),
    clear: () => remove(key),
  };
}

function boolEntry(key: string, fallback: boolean): KvEntry<boolean> {
  return {
    key,
    get: () => {
      const value = read(key);
      if (value === null) {
        return fallback;
      }
      return value === "1" || value === "true";
    },
    set: (value) => write(key, value ? "1" : "0"),
    clear: () => remove(key),
  };
}

export type SortDirection = "asc" | "desc";

export type StoredSort = {
  column: string;
  direction: SortDirection;
};

function parseStoredSort(
  raw: string | null,
  allowed: readonly string[],
  fallback: StoredSort,
): StoredSort {
  if (raw == null) {
    return fallback;
  }

  const [column, direction] = raw.split(":");

  if (!allowed.includes(column)) {
    return fallback;
  }

  if (direction !== "asc" && direction !== "desc") {
    return fallback;
  }

  return { column, direction };
}

function sortEntry(
  key: string,
  allowed: readonly string[],
  fallback: StoredSort,
): KvEntry<StoredSort> {
  return {
    key,
    get: () => parseStoredSort(read(key), allowed, fallback),
    set: (value) => write(key, `${value.column}:${value.direction}`),
    clear: () => remove(key),
  };
}

/** App-wide persisted keys. */
export const kv = {
  theme: stringEntry("nimbus-theme", "dark"),
  editorVim: boolEntry("nimbus-editor-vim", false),
  editorWrap: boolEntry("nimbus-editor-wrap", true),
  previewFilter: stringEntry("nimbus-preview-filter", "all"),
  previewAutoplay: boolEntry("nimbus-preview-autoplay", true),
  filesSort: sortEntry(
    "nimbus-files-sort",
    ["kind", "name", "size", "created"],
    { column: "kind", direction: "asc" },
  ),
  usersSort: sortEntry(
    "nimbus-users-sort",
    ["username", "email", "createdAt", "updatedAt"],
    { column: "username", direction: "asc" },
  ),
} as const;
