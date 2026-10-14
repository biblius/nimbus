import { kv } from "./storage";

export type Theme = "dark" | "light";

const themeColor: Record<Theme, string> = {
  dark: "#171717",
  light: "#f9fafb",
};

export const theme = $state<{ mode: Theme }>({ mode: "dark" });

function storedTheme(): Theme {
  return kv.theme.get() === "light" ? "light" : "dark";
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
  kv.theme.set(next);
  applyTheme(next);
}
