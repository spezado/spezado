// SPDX-License-Identifier: AGPL-3.0-or-later
// Amounts and dates as people of a language write them, and back.

import type { Lang } from "./i18n";

/** An amount from the API (`"-1234.50"`) as people write it: `-1.234,50` or `-1,234.50`. */
export function amount(value: string, lang: Lang): string {
  const negative = value.startsWith("-");
  const [whole = "0", fraction] = value.replace("-", "").split(".");
  const [group, point] = lang === "de" ? [".", ","] : [",", "."];
  const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, group);
  return `${negative ? "-" : ""}${grouped}${fraction !== undefined ? point + fraction : ""}`;
}

/** An amount typed – with or without separators of thousands – for the API: `"1234.50"`; null if it is none. */
export function parseAmount(text: string, lang: Lang): string | null {
  let plain = text.trim().replace(/[\s €]/g, "");
  plain = lang === "de" && plain.includes(",") ? plain.replace(/\./g, "").replace(",", ".") : plain.replace(/,/g, "");
  return /^-?\d+(\.\d{1,2})?$/.test(plain) ? plain : null;
}

/** A date from the API (`YYYY-MM-DD`) as people write it. */
export function date(value: string, lang: Lang): string {
  if (lang === "en") return value;
  const [year, month, day] = value.split("-");
  return `${day}.${month}.${year}`;
}

/** Today, `YYYY-MM-DD`, by the device's clock. */
export function today(): string {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}
