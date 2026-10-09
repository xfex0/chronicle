import { createContext, useContext, type ReactNode } from "react";
import type { Language } from "../api/types";
import { en, type Dict } from "./en";
import { uk } from "./uk";

export type Key = keyof typeof en;
const dicts: Record<Language, Dict> = { en, uk };

export function detectLanguage(): Language {
  return typeof navigator !== "undefined" && navigator.language.toLowerCase().startsWith("uk") ? "uk" : "en";
}

export function translate(lang: Language, key: Key, vars?: Record<string, string | number>): string {
  let s: string = dicts[lang][key] ?? en[key];
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}

type T = (key: Key, vars?: Record<string, string | number>) => string;
const Ctx = createContext<{ lang: Language; t: T }>({ lang: "en", t: (k, v) => translate("en", k, v) });

export function I18nProvider({ lang, children }: { lang: Language; children: ReactNode }) {
  return <Ctx.Provider value={{ lang, t: (k, v) => translate(lang, k, v) }}>{children}</Ctx.Provider>;
}

export const useI18n = () => useContext(Ctx);

/** Turn a backend error code into a translated message. */
export function errorText(t: T, code: string): string {
  const base = code.split(":")[0] ?? code;
  const key = `err.${base}` as Key;
  return key in en ? t(key) : t("err.generic", { message: code });
}

/** Dates arrive as "1337.4.1" or "-304"; show BC years readably. */
export function formatDate(lang: Language, d: string | null): string {
  if (!d) return "—";
  if (d.startsWith("-")) return lang === "uk" ? `${d.slice(1).split(".")[0]} р. до н. е.` : `${d.slice(1).split(".")[0]} BC`;
  return d;
}
