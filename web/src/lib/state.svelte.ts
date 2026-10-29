// SPDX-License-Identifier: AGPL-3.0-or-later
// The state of the web app: language, appearance, the books shown, the year, the page – and a counter that grows
// whenever the books change, so that pages read again.

import { catalog, type Key, type Lang } from "./i18n";

export const pages = ["overview", "journal", "bank", "documents", "invoices", "accounts", "reports", "review", "closing", "book"] as const;
export type Page = (typeof pages)[number];
export type Theme = "auto" | "light" | "dark";

function stored(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function store(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Kept for this visit only.
  }
}

function page(): Page {
  const hash = location.hash.replace(/^#\/?/, "");
  return (pages as readonly string[]).includes(hash) ? (hash as Page) : "overview";
}

const browser: Lang = navigator.language.toLowerCase().startsWith("de") ? "de" : "en";

export const app = $state({
  lang: (stored("spezado.lang") as Lang | null) ?? browser,
  theme: (stored("spezado.theme") as Theme | null) ?? "auto",
  books: null as string | null,
  year: new Date().getFullYear(),
  page: page(),
  version: 0,
});

addEventListener("hashchange", () => (app.page = page()));

/** A text in the language shown. */
export function t(key: Key): string {
  return catalog[app.lang][key];
}

export function go(to: Page): void {
  location.hash = `#/${to}`;
}

export function setLang(lang: Lang): void {
  app.lang = lang;
  document.documentElement.lang = lang;
  store("spezado.lang", lang);
}

export function setTheme(theme: Theme): void {
  app.theme = theme;
  if (theme === "auto") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
  store("spezado.theme", theme);
}
