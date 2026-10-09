export type Theme = "dark" | "light";

const STORAGE_KEY = "nimbus-theme";

const themeColor: Record<Theme, string> = {
  dark: "#171717",
  light: "#f9fafb",
};

export const theme = $state<{ mode: Theme }>({ mode: "dark" });

function storedTheme(): Theme {
  try {
    return localStorage.getItem(STORAGE_KEY) === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

export function applyTheme(mode: Theme) {
  theme.mode = mode;
  document.documentElement.classList.toggle("light", mode === "light");
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", themeColor[mode]);
}

export function initTheme() {
  applyTheme(storedTheme());
}

export function toggleTheme() {
  const next: Theme = theme.mode === "dark" ? "light" : "dark";
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {
    // Preference stays for this page when storage is unavailable.
  }
  applyTheme(next);
}
