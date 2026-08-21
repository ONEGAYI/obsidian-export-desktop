import { useEffect, useState } from "react";

export type Theme = "dark" | "light";

const STORAGE_KEY = "obsidian-export-theme";

export function initialTheme(): Theme {
  const stored = localStorage.getItem(STORAGE_KEY);
  return stored === "light" ? "light" : "dark";
}

function applyTheme(theme: Theme) {
  document.documentElement.classList.toggle("dark", theme === "dark");
}

export function useTheme(): [Theme, (theme: Theme) => void] {
  const [theme, setTheme] = useState<Theme>(initialTheme);

  useEffect(() => {
    applyTheme(theme);
    localStorage.setItem(STORAGE_KEY, theme);
  }, [theme]);

  return [theme, setTheme];
}
