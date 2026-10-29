# CLAUDE.md – spezado

Rules for working in this repository. This file is public: nothing personal goes here.

## What this is

spezado – free accounting for private persons, sole traders and freelancers, companies and non-profits. One
monorepo for the Rust core, CLI/TUI, client, relay, backend, desktop app, progressive web app and the European law
modules. The data model and protocol live in [librotenado](https://github.com/librotenado/librotenado) (MIT library,
CC BY 4.0 specification); the design is in [docs/spec.md](docs/spec.md).

Version 0.3.0, a preview (`CHANGELOG.md`): milestones 0 to 3 and 5 – core, terminal interface, bank, API, web app and
synchronisation – and `spezado tray` with autostart, on
librotenado 0.6 (tag `v0.6.0`). The crates and what goes where are listed in CONTRIBUTING.md; the plan is in
[docs/spec.md](docs/spec.md). Releases: a tag `v*` runs `.github/workflows/release.yml` – binaries with the web app
embedded for Linux, macOS and Windows, published as a pre-release with the notes of `CHANGELOG.md`.

Documentation (`docs/`, mdBook, published to spezado.dev by `.github/workflows/docs.yml`): guides in `docs/src/`;
the design (`docs/spec.md`) and the changelog are only included there – each has one home. Keep the guides in step
with the terminal, the web app, the command line and the API.

- `spezado-core` – pure: `Books` over librotenado's `EventSet`, `Command` → checked → one event payload, journal,
  review list. No I/O, no clock.
- `spezado-law` / `spezado-law-de` – the `LawModule` trait and the German module (DATEV charts, tax table, reports
  per year, VAT periods). Which national chart an owner works with is part of its configuration (librotenado 0008).
- `spezado-store` – one SQLite database for all books of a workspace (events in canonical form, settings),
  originals as files named by hash.
- `spezado-app` – `Workspace`: stamps (HLC, chain), stores and applies commands all or none, also across books;
  set-up, bookings across books (`transaction`), reports, export/import as JSON Lines; `opening.rs` opening balances
  against the opening balance sheet, `review.rs` the review list in words (German, English), `invoicing.rs` the
  seller's details and customers (settings per books), invoice numbers and invoices written as XRechnung (UBL).
- `spezado-tui` – the terminal interface (ratatui): `app.rs` is the state and what each key and click does (tested
  without a terminal, `TestBackend`), `view.rs` draws it and returns what was drawn where (`Hits`) – a click is
  mapped to the key or row under it –, `form.rs` the forms and the account search, `i18n.rs` every text in German and
  English – a text missing a language does not build. The mouse is on by default, `m` switches it off (the
  terminal's own text selection then works without a modifier key).
- `spezado-app/src/bank.rs` – statements imported (one database document each, a bank account per IBAN opened on
  first sight), open entries with proposals (the payment booked before the statement – reversed and booked again
  on the bank account –, an unbooked invoice whose amount due matches, else the same party's last booking), booking
  entries, FinTS as a placeholder that waits for a product registration number (setting
  `fints-product`).
- `spezado-api` – the open API (axum): `spezado serve` on 127.0.0.1 (default port 8048), `/api/v1`, pairing by a
  code shown at start for a token per device (stored as its hash in the workspace setting `api-devices`), Server-Sent
  Events `/api/v1/events`, the workspace behind a mutex and `spawn_blocking`. Plain JSON types in `dto.rs` (amounts
  as `{value: "12.40", currency}`), routes with their OpenAPI in `routes.rs`, documents, bank and GoBD export in
  `records.rs` (files up to 32 MB as the raw body; a document read waits an hour for its decision under a pending
  id); `api/openapi.json` is written by `cargo run -- openapi > api/openapi.json` and a test fails when it is out of
  date. The web app is embedded from
  `web/dist` (rust-embed, a strict Content-Security-Policy, any other path is the app); without a build the API is
  served alone.
- `web/` – the web app: Svelte 5 + Vite without SvelteKit, TypeScript 6, pnpm, Node 26. `pnpm api` writes
  `src/lib/api-types.ts` from `api/openapi.json` (CI checks it is current), `pnpm check` (svelte-check, no
  warnings), `pnpm build` → `dist/`. Own small components (the account search is a WAI-ARIA combobox), texts in
  `src/lib/i18n.ts` – English must have every German key –, the brand in `src/brand.css`, a hash router
  (`#/overview`, `#/journal`, `#/bank`, `#/documents`, `#/invoices`, `#/accounts`, `#/reports`,
  `#/review`, `#/closing`, `#/book`), live updates by Server-Sent Events. Develop with `spezado serve` running and
  `pnpm dev`, which passes `/api` on to it. `spezado` – the command line; `spezado tui` creates a workspace on
  first start.

## Architecture

- **One core, many interfaces:** the terminal interface, the API and the command line work through `spezado-app`
  only. The web app (Svelte 5, TypeScript, served by the binary) and the Flutter apps use the API.
- **librotenado stays protocol only:** what every implementation must read, write and check the same way goes there
  (and may change while it is a draft); how spezado works stays here.
- **Books stay apart, linked by transactions:** one set of books per organisation or person, all in one database;
  a business transaction across books is one booking in each, linked by `transaction`, written in one transaction.

## Principles

- **Everything included:** one edition, every feature free; no paid tiers, licence keys or unlocks. External law
  modules must be free software under a licence compatible with the AGPL – never proprietary.
- **No telemetry.** Only connections that are technically needed or that users start themselves.
- **Local first:** the client holds the full core and works offline; relays only see encrypted events, a backend is
  a trusted peer.
- **Events are the truth:** bookings are never changed, only reversed; every period records the module versions it
  was computed with.
- **Our own model is the reference;** existing formats (EN 16931, SAF-T, camt, XBRL, national charts of accounts)
  are translations. The original document stays the receipt.
- **Money is never a float.** Multiple currencies from day one.

## Brand

The brand kit lives in its own repository, [spezado/brand](https://github.com/spezado/brand) (generator MIT, artwork
CC BY-ND 4.0, Domitian OFL-1.1): two folder spines, ink and copper bronze as equals; wordmark in Domitian. `brand/`
here holds copies of what spezado uses – the fonts for invoice PDFs, `web/` for the documentation, the lockups for the
README. Change the brand there and copy; never edit the copies.

## Licences

- Everything here: **AGPL-3.0-or-later**, an SPDX header in every own source file. Exceptions (see `REUSE.toml`):
  the description of the API, `api/openapi.json`, is **MIT**, so that clients generated from it can be licensed
  freely; the documentation's text in `docs/` is **CC BY 4.0**, like librotenado's specification, so that text moves
  freely between them; the brand's artwork is CC BY-ND 4.0 and the Domitian fonts OFL-1.1, as in spezado/brand.
- Never copy core or business logic into the MIT repositories: the librotenado library, and the Flutter apps in
  `spezado/apps`, which talk to spezado through its open API only.

## Rules

- `cargo fmt`, `cargo clippy --all-targets` (no warnings) and `cargo test` before every commit; keep
  `docs/spec.md` in step with what is decided. After changing dependencies: exact versions at least 14 days old
  (`scripts/check-age.py`, pnpm's `minimumReleaseAge`) and `cargo deny check`.
- English in code, docs and commits; commits signed off (`git commit -s`).
- No personal data in tracked files (names, paths like `/Users/…`, hostnames, keys).
- Claims need a source; unverified statements are marked **OPEN**. Legal and tax content is a draft, not advice.
- No externally loaded fonts or scripts in anything web-based.
