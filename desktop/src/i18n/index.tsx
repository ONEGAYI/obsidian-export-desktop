import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";

import { en } from "./en";
import { zh, type Dict } from "./zh";

/** What the user picked: a concrete language, or "follow the OS setting". */
export type LanguagePreference = "zh" | "en" | "system";
/** The language actually applied to the UI. */
export type ResolvedLanguage = "zh" | "en";

export const LANGUAGE_ORDER: LanguagePreference[] = ["zh", "en", "system"];

const STORAGE_KEY = "obsidian-export-language";

const DICTIONARIES: Record<ResolvedLanguage, Dict> = { zh, en };

export function initialLanguagePreference(): LanguagePreference {
  const stored = localStorage.getItem(STORAGE_KEY);
  return LANGUAGE_ORDER.includes(stored as LanguagePreference)
    ? (stored as LanguagePreference)
    : "system";
}

/**
 * Any Chinese locale (zh, zh-CN, zh-TW, …) resolves to the zh dictionary;
 * everything else falls back to English.
 */
function systemLanguage(): ResolvedLanguage {
  const langs = navigator.languages?.length
    ? navigator.languages
    : [navigator.language];
  return langs.some((l) => l.toLowerCase().startsWith("zh")) ? "zh" : "en";
}

/** Replace `{name}` placeholders in a dictionary template string. */
export function fmt(
  template: string,
  params: Record<string, string | number>,
): string {
  return template.replace(/\{(\w+)\}/g, (match, key: string) =>
    key in params ? String(params[key]) : match,
  );
}

interface I18nContextValue {
  preference: LanguagePreference;
  resolved: ResolvedLanguage;
  setPreference: (language: LanguagePreference) => void;
  /** Dictionary for the resolved language; access entries as `t.app.export`. */
  t: Dict;
}

const I18nContext = createContext<I18nContextValue | null>(null);

/**
 * Language state with OS-following support, exposed through the project's
 * first React Context so every view reads the same dictionary without
 * prop-drilling. Mirrors the theme hook (`lib/theme.ts`): the preference is
 * persisted as a bare localStorage string, and a "system" preference derives
 * the resolved language from `navigator.languages` once at startup (there is
 * no standard event for locale changes mid-session).
 */
export function I18nProvider({ children }: { children: ReactNode }) {
  const [preference, setPreference] = useState<LanguagePreference>(
    initialLanguagePreference,
  );
  const [system] = useState<ResolvedLanguage>(systemLanguage);

  const resolved: ResolvedLanguage =
    preference === "system" ? system : preference;

  useEffect(() => {
    document.documentElement.lang = resolved;
    localStorage.setItem(STORAGE_KEY, preference);
  }, [preference, resolved]);

  const value: I18nContextValue = {
    preference,
    resolved,
    setPreference,
    t: DICTIONARIES[resolved],
  };

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    throw new Error("useI18n must be used within an I18nProvider");
  }
  return ctx;
}
