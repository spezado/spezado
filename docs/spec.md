# spezado – design

The design of spezado: what it is for, how it is built and in what order. The protocol it is built on is specified
in [librotenado](https://github.com/librotenado/librotenado) and explained in its
[whitepaper](https://github.com/librotenado/whitepaper). Statements marked **OPEN** are not decided or not verified.

## 1. What it is

Free accounting for everyone:

- **private persons** – down to household finances, such as looking at one's own purchases on one's own computer;
- **sole traders and freelancers;**
- **companies** – GmbH, AG and other legal forms;
- **non-profits** – associations, foundations, non-profit companies.

One person has one account and works in **as many organisations as they have access to**, with roles per
organisation (owner, bookkeeper, tax adviser, auditor – **OPEN**: the final list).

## 2. Principles

- **Everything included** – one edition, every feature free; no paid tiers, licence keys or unlocks.
- **No telemetry.**
- **Local first** – the full core runs on the user's device and works offline.
- **Events are the truth** – bookings are never changed, only reversed; balances and reports are derived.
- **librotenado is the model** – existing formats are translations; the original document stays the receipt.
- **Visible conflicts** – whatever breaks a rule goes to a review list; nothing is resolved silently.

## 3. Scope

| Area | Content | When |
|---|---|---|
| Bookkeeping | bookings, accounts, multiple currencies from day one, periods, closing | first |
| Bank | statement import (camt, MT940), then a direct bank connection – early, so that receipts can be matched to real payments | first |
| Receipts | automatic recognition: embedded e-invoice (ZUGFeRD/Factur-X, XRechnung) first, then the text layer, then OCR | first |
| Invoices | writing e-invoices (EN 16931), number ranges per device | first |
| Reports | balances, profit and loss, cash-basis accounts, balance sheet, sphere accounts for non-profits | first |
| Filings | VAT returns and annual filings through adapters (ELSTER/ERiC first) | Germany |
| Payroll | wage tax, social security | Germany, before other countries |
| Other countries | Austria and Switzerland, then further European countries | after Germany is complete |

## 4. Operation

One core, three roles, one protocol:

- **Client** – the full core on a device: standalone, on the command line and in a terminal interface, and behind
  the open API for the web app and the apps.
- **Relay** – stores and forwards events it cannot read (end-to-end encrypted); anyone can run one for themselves or
  for others.
- **Server** – a trusted peer that processes plain text: recognition, scheduled bank retrieval, filings, accounts and
  roles of a team.

## 5. Architecture

- **Rust** for the core, the command line and terminal interface, the API, relay and server – one binary.
- **The open API** is the line between spezado and everything around it: HTTP and JSON, described in OpenAPI from the
  code, with generated clients. A spezado on the same device, in the local network or on a server; devices pair once
  by code or QR code and get a token each, which can be revoked (**OPEN**: accounts on servers – passkeys, OIDC).
- **The web app:** Svelte 5 and TypeScript, served by the binary as a static single-page app (TypeScript 6 until
  Svelte's tools support TypeScript 7).
- **Desktop and mobile apps:** Flutter, MIT, in their own repository ([spezado/apps](https://github.com/spezado/apps)),
  talking to spezado through the API only – they contain neither the core nor the librotenado library, and can be
  distributed through app stores.
- **Storage:** one SQLite database per workspace holds every set of books, so that books that belong together are
  written in one transaction; originals as files named by their hash. On a server PostgreSQL and S3-compatible
  storage (**OPEN**). Archives in open formats; each set of books can be exported and imported on its own.
- **Books stay apart:** one set of books per organisation or person – the unit of sharing and encryption. A business
  transaction across books (a bill partly private) is one booking in each, linked by a `transaction` (librotenado
  0002) and written together.
- **Synchronisation:** a growing set of events, hybrid logical clocks, a hash chain per device, a review list
  (librotenado 0002); encryption for groups after librotenado 0006.

### Crates

| Crate | Content |
|---|---|
| `spezado-core` | the books over librotenado's event set; commands checked into events; journal and review list – pure, no I/O |
| `spezado-law` | the interface of law modules: facts of an owner in, charts, tax codes, reports and periods out |
| `spezado-law-de` | German law |
| `spezado-store` | events in SQLite, originals as files |
| `spezado-app` | the workspace: stamping, storing and applying commands all or none, workflows, reports, export and import |
| `spezado-tui`, `spezado-api` | the terminal interface and the API, through `spezado-app` only |
| `spezado` | the binary: the command line, `tui`, `serve` |

## 6. Law as modules

- **Building blocks** instead of one module per country: country × legal form × additions (non-profit, small
  business) × region.
- **Pure functions:** facts in – who the owner is and how it keeps its books (librotenado 0008) –, derived results
  out (charts, tax codes, reports, periods, deadlines); no I/O, no state.
- **Data before code:** rates, charts of accounts and form mappings are tables with validity periods, published by
  librotenado.
- **Versions per period:** closing a period records the modules and versions its figures were derived with.
- **Rust, built two ways:** spezado's European modules are compiled in; external modules are loaded as WebAssembly
  components in a sandbox (**OPEN**).
- **Never proprietary:** the module interface is AGPL; a module declares its licence and source in a manifest and
  must be free software compatible with the AGPL.
- **Tested against official cases** (ERiC test data, wage tax test tables, XRechnung examples) and reviewed by
  someone who knows the law, not only the code – all German mappings by tax advisers before the first stable
  release.

## 7. Licences

- spezado: **AGPL-3.0-or-later**, distributed outside app stores.
- The apps: **MIT**, in their own repository, through the API only.
- librotenado: library **MIT**, specification **CC BY 4.0**.
- **OPEN:** review of the split along the API by someone who knows licence law.

## 8. Milestones

| | Content | State |
|---|---|---|
| M0 Core | the crates above, SQLite, commands and queries, the command line | done |
| M1 Terminal interface | a freelancer books a year: cash basis, VAT returns, SKR 03/04, e-invoices read with booking proposals, reversals, GoBD export | done |
| M2 Bank | bank statements (camt.053, MT940, CSV, read in librotenado), booking their entries, proposals from invoices and earlier entries; FinTS as a placeholder | done |
| M3 API and web app | `spezado serve`, OpenAPI, the web app | done |
| 0.1.0 | a preview for a freelancer's year: opening balances, VAT deadlines with public holidays and the permanent extension, payments booked before their statement, invoices written as XRechnung or ZUGFeRD; binaries per platform | released |
| M4 Apps | Flutter desktop and mobile apps in spezado/apps | |
| M5 Sync | devices, relay, end-to-end encryption (librotenado 0006), sharing with rights, TLS to other devices | done, released as 0.2.0 |
| then | ELSTER (ERiC), E-Bilanz (XBRL), payroll | |

## 9. State

Milestone 5, first stage – a person's own devices, after librotenado 0006:

- `spezado relay` holds, per set of books, the ordered member list and the envelopes of events and originals, and
  checks signatures, the rules of the member list and who may write – never content. It speaks HTTP behind a reverse
  proxy that terminates TLS.
- A device **starts** synchronising books: it founds their member list with itself and a **recovery code** – shown
  once, for paper – and sends every event and original, encrypted with the books key.
- Another device of the person **joins** with an invitation (relay, books, a secret; good for a day); the inviting
  device adds it at its next synchronisation, after checking the proof. The new device unwraps the key and the chain
  and receives everything.
- When every device is lost, the recovery code **recovers** the books onto a new one.
- `spezado serve` synchronises every minute; the terminal with `y`, the command line with `spezado sync`.
- Second stage: other people with rights (`read`, `write`, `manage`) by invitation, removing a device or a person (the
  key changes), read-only books refuse commands; seeds in the key store of the system; the seller's details,
  customers and series of invoice numbers per device as events (librotenado 0.6); `serve --listen` for other
  devices over TLS with a certificate of the workspace's own, its fingerprint in the pairing link and a QR code.
- **OPEN:** access for a limited time, an auditor for one period.


Version 0.1.0, a preview – for trying out, not for a real tax return: the German mappings are drafts until tax
advisers have reviewed them. On top of milestones 0 to 3 (version 0.2.0 adds milestone 5, above):

- **Opening balances**, booked once against the opening balance sheet (librotenado 0004), so that statements close
  at the books' balance; balances carry on between years by themselves.
- **VAT deadlines** move on public holidays of the state where the tax office sits (§ 108 Abs. 3 AO; holidays of
  only some municipalities left out) and by a month with the permanent extension (§ 46 UStDV); both are part of the
  configuration (librotenado 0008) and can be changed from a date on, in the terminal, the API and the web app.
  **OPEN:** whether the current AEAO to § 108 names the seat of the tax office.
- **Payments booked before their statement**: the first proposal for an entry; accepting it reverses the booking and
  books it again on the statement's bank account, naming the entry, on the same day.
- **Invoices** written as XRechnung 3.0 (UBL) or as ZUGFeRD (profile XRECHNUNG): a PDF/A-3b set in Domitian with the
  XRechnung in CII attached, accepted by veraPDF and the Mustang validator; the file written is the original kept. The
  seller's details, customers, numbers in sequence per year, lines at
  19, 7 or 0 % VAT, exempt (§ 4 UStG, with the reason), under reverse charge at home (§ 13b UStG) or as a service to a
  business in another EU country (§ 3a Abs. 2 UStG), intra-Community supplies and exports – each with the statement
  § 14 Abs. 4 Nr. 8 and § 14a UStG ask for and the VAT identification numbers they need –, or as a small business
  (§ 19 UStG); booked when paid on a cash basis, when issued with double entry. Without a VAT identification number
  the seller's tax number is also its identifier (BT-29): EN 16931 requires BT-29, BT-30 or BT-31 (BR-CO-26, fatal).
  The seller's details and customers travel with the books; every device writes its own series of numbers.
- **The review list in words**, German and English: each issue names the booking, document, entry and amount.
- Releases: binaries for Linux (x86_64, arm64), macOS (arm64) and Windows (x86_64) with the web app embedded.

Milestone 3: `spezado serve` – the open API on the device itself, described in
[api/openapi.json](https://github.com/spezado/spezado/blob/main/api/openapi.json): pairing, books and their set-up, overview, journal, accounts, balances, tax
codes, bookings from a gross amount and in lines, reversals, documents (read with a booking proposal, filed with or
without a booking, the original fetched again), bank statements and their open entries (booked by hand, by a proposal,
all proposals at once; FinTS answers 501 until registered), reports, the review list, closing, the GoBD export as a
ZIP, and Server-Sent Events when something changed. And the web app, served by the same binary, with all of it:
pairing by the address `spezado serve` shows, setting up books, overview, journal, bank, documents, accounts with
their ledger, reports with a CSV download, the review list and closing – German and English, light and dark, live when
the books change. Next: the Flutter apps (M4) – they need the API from other devices, with TLS.

Milestone 2: the tab Bank – statements read from files in camt.053, MT940 or a bank's CSV, a bank account opened for
each IBAN, statements read twice and entries two statements show left out; each entry booked from its gross amount
with an account and a tax code, or by a proposal: the unbooked invoice it pays, or the account of the same party's
last entry; all proposals at once. The review list shows entries booked twice and statements that do not close at
their account's balance. Fetching from the bank (FinTS) is a placeholder: it needs a product registration with the
Deutsche Kreditwirtschaft, which falkOSS applies for once it is founded (**OPEN**).

Milestone 1: the terminal interface (`spezado tui`), in German and English, by keyboard and mouse – setting up a
freelancer's books (cash basis or double entry, standard or small-business VAT, SKR 03 or 04), booking from a gross amount and a tax code or
in lines, reversing, reading e-invoices (XRechnung, ZUGFeRD) into booking proposals and filing other documents,
balances and ledgers, the advance VAT returns with their deadlines and the Anlage EÜR, the review list, closing
periods and the data transfer of the GoBD (Z3). **OPEN:** reading the text layer and photos of documents.

Milestone 0: `spezado init`, `set-up`, `book`, `reverse`, `journal`, `balance`, `report`, `close`, `check`,
`export` and `import` on a workspace. German reports as librotenado publishes them: the VAT return of 2026, the
Anlage EÜR of 2025 and 2026, the HGB statements of 2026.
