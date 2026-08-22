import { useEffect, useState } from "react";

/** What the user picked: a concrete theme, or "follow the OS setting". */
export type ThemePreference = "light" | "dark" | "system";
/** The theme actually applied to the document. */
export type ResolvedTheme = "light" | "dark";

export const THEME_ORDER: ThemePreference[] = ["light", "dark", "system"];

const STORAGE_KEY = "obsidian-export-theme";

export function initialThemePreference(): ThemePreference {
  const stored = localStorage.getItem(STORAGE_KEY);
  // Values written by older builds ("light"/"dark") keep their meaning.
  return THEME_ORDER.includes(stored as ThemePreference)
    ? (stored as ThemePreference)
    : "system";
}

function systemTheme(): ResolvedTheme {
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

/**
 * Theme state with OS-following support. `preference` is the stored choice;
 * when it is "system", the resolved theme tracks `prefers-color-scheme`
 * live, so a system-wide switch recolors the window without a restart.
 */
export function useTheme(): [
  ThemePreference,
  ResolvedTheme,
  (theme: ThemePreference) => void,
] {
  const [preference, setPreference] = useState<ThemePreference>(
    initialThemePreference,
  );
  const [system, setSystem] = useState<ResolvedTheme>(systemTheme);

  useEffect(() => {
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (event: MediaQueryListEvent) =>
      setSystem(event.matches ? "dark" : "light");
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, []);

  const resolved: ResolvedTheme = preference === "system" ? system : preference;

  useEffect(() => {
    document.documentElement.classList.toggle("dark", resolved === "dark");
    localStorage.setItem(STORAGE_KEY, preference);
  }, [preference, resolved]);

  return [preference, resolved, setPreference];
}
