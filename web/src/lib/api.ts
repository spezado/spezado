// SPDX-License-Identifier: AGPL-3.0-or-later
// The client of spezado's API, typed from api/openapi.json, with the token of this device.

import createClient, { type Middleware } from "openapi-fetch";
import type { components, paths } from "./api-types";

export type Schemas = components["schemas"];

const TOKEN = "spezado.token";

/** The token of this device, if it is paired; kept in this browser only. */
export function token(): string | null {
  try {
    return localStorage.getItem(TOKEN);
  } catch {
    return null;
  }
}

export function setToken(value: string | null): void {
  try {
    if (value === null) localStorage.removeItem(TOKEN);
    else localStorage.setItem(TOKEN, value);
  } catch {
    // A browser that keeps nothing pairs again next time.
  }
}

const authorise: Middleware = {
  onRequest({ request }) {
    const current = token();
    if (current) request.headers.set("Authorization", `Bearer ${current}`);
    return request;
  },
};

export const api = createClient<paths>({ baseUrl: "" });
api.use(authorise);

/** What went wrong in an answer: its `error`, or the status. */
export function problem(error: unknown, response: Response): string {
  const text = (error as { error?: string } | undefined)?.error;
  return text ?? `${response.status} ${response.statusText}`;
}

/** Calls `changed` whenever the books change, until the returned function is called. */
export function onChange(changed: (books: string) => void): () => void {
  const current = token();
  if (!current) return () => {};
  const source = new EventSource(`/api/v1/events?token=${encodeURIComponent(current)}`);
  source.addEventListener("changed", (event) => {
    const data = JSON.parse((event as MessageEvent<string>).data) as { books: string };
    changed(data.books);
  });
  return () => source.close();
}

/** Sends a file to the API as it is; returns the answer's JSON or what went wrong. */
export async function upload<T>(path: string, file: Blob): Promise<{ data: T | null; failure: string | null }> {
  const headers: Record<string, string> = { "Content-Type": "application/octet-stream" };
  const current = token();
  if (current) headers.Authorization = `Bearer ${current}`;
  const response = await fetch(path, { method: "POST", headers, body: file });
  const body = (await response.json().catch(() => null)) as (T & { error?: string }) | null;
  if (!response.ok) return { data: null, failure: body?.error ?? `${response.status} ${response.statusText}` };
  return { data: body, failure: null };
}

/** Fetches a file from the API and hands it to the browser to save; returns what went wrong. */
export async function download(path: string, name: string): Promise<string | null> {
  const current = token();
  const response = await fetch(path, { headers: current ? { Authorization: `Bearer ${current}` } : {} });
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string } | null;
    return body?.error ?? `${response.status} ${response.statusText}`;
  }
  save(await response.blob(), name);
  return null;
}

/** Hands bytes to the browser to save under a name. */
export function save(blob: Blob, name: string): void {
  const url = URL.createObjectURL(blob);
  const link = Object.assign(document.createElement("a"), { href: url, download: name });
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
