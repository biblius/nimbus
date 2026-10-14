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

/** App-wide persisted keys. */
export const kv = {
  theme: stringEntry("nimbus-theme", "dark"),
  editorVim: boolEntry("nimbus-editor-vim", false),
  editorWrap: boolEntry("nimbus-editor-wrap", true),
} as const;
