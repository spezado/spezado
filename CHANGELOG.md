# Changelog

## Unreleased

- **`spezado-tray` on Windows:** the server with its icon in the system tray as a program of its own, without a
  console window; it starts at login (`Start at login`, `spezado autostart on`).
- **Tax codes of automatic accounts:** choosing an account of SKR 03 or SKR 04 on which DATEV computes VAT (AM, AV)
  proposes its tax code – `8400` `de-out-19`, `3736` `de-in-19` –, in the terminal, the web app and the API
  (`tax` of an account).
- **Cash discounts:** the accounts of cash discounts granted and received (SKR 03 3730 ff., 8730 ff.; SKR 04 5730 ff.,
  4730 ff.) are there; librotenado had left them out.
- **Remaining terms** on the balance sheet (§ 268 Abs. 4 and 5 HGB): per position, what is due within a year and
  after more than a year, in the command line, the web app and the API.
- **Bookings in one balance sheet only** (`basis` of a booking in lines, API) and the **reconciliation to the tax
  balance sheet** with the balance sheet.
- **Access for a time:** `spezado sync invite --until`, and `until` in the API; a device that may manage the books
  removes members whose time ran out. Names of devices are sealed from the relay, and envelopes padded; the relay
  serves a member up to its last day.
- **A financial year from the middle of a month:** `--year-start-day`.
- **Payments for third parties** of an XRechnung booked on transit items (SKR 03 1590, SKR 04 1370).
- Public holidays checked against the law of every state; Berlin's 17 June 2028 added.
- librotenado 0.16.0.

- **A contact for a document recorded already:** in the web app's list of documents, and the API; a correction of
  the document after librotenado 0005, its original unchanged. Invoices written name their customer as a contact.
- **Shortened balance sheets** for small and micro corporations (§ 266 Abs. 1 HGB): `de-hgb-266-small` and
  `de-hgb-266-micro`.
- Bank files in **camt.052 and camt.054** are read like camt.053.
- The data transfer of the GoBD names the accounts' numbers of SKR 03 or SKR 04.
- **Fixed assets** after librotenado 0011: register an asset with its cost and how it is depreciated – on a straight
  line by month, at once, in a pool, or not at all –, book the depreciation of a year once, and book an asset out at
  its book value when it leaves; the report **Anlage AVEÜR**. In the terminal (*Accounts*: `a`, `f`), the web app
  (*Assets*) and the API.
- **The annual VAT return** (`de-ust`, Umsatzsteuererklärung 2026) as a report and a filing over the calendar year.
- **Business entertainment** booked from a gross amount is split: 70 % on the account of entertainment, 30 % on the
  one not deductible, the VAT in full (§ 4 Abs. 5 Satz 1 Nr. 2 EStG, § 15 Abs. 1a UStG).
- **Ten days around the turn of the year:** a regularly recurring payment – the VAT of the last quarter, paid on
  8 January – can count in the year it belongs to (§ 11 EStG); in the web app and the API.
- The tax codes for the exempt letting of land (§ 4 Nr. 12 UStG) and for new vehicles supplied to buyers without a VAT
  identification number, in bookings and – letting – invoices.
- **Contacts – customers and suppliers** after librotenado 0008 (0.13): with a number (customers from 10001,
  suppliers from 70001), address, e-mail and phone, VAT identification and tax number, bank account and contact
  persons; in the terminal (tab 9), the web app (*Contacts*) and the API. The customers of the invoice form are the
  contacts that are customers. **Reading an e-invoice offers its seller or buyer** among the contacts – by VAT
  identification number, tax number, IBAN or name, the most certain first – or keeps it as a new contact; the document
  names the contact chosen. The review list names a number given to two contacts.
- **The recapitulative statement** (Zusammenfassende Meldung, § 18a UStG) as a report and a filing, `de-zm`: supplies
  to businesses in other EU member states per customer's VAT identification number, after librotenado 0009. The
  review list names a booking it cannot place – no VAT identification number of the customer, or several. The
  reports list its quarters – or months, above 50,000 euros of goods – with something to report and their deadlines.
- **Filings** after librotenado 0007: the advance VAT return and the Anlage EÜR prepared from the books – again as
  a correction once one was submitted, outdated when the books change –, submitted through the adapter of their
  country, also as a test, and asked about; what the authority returns becomes a document. In the terminal, the web
  app, the API and with `spezado filing` and `spezado adapter`. There is no adapter for ELSTER yet (**OPEN**).
- **A container image** for servers with every release, `ghcr.io/spezado/spezado` for Linux on amd64 and arm64: it
  runs the relay unless told otherwise; `deploy/compose.yaml` starts one behind a reverse proxy.
- The documentation at spezado.dev is built with Jekyll and Just the Docs, as falkOSS projects build theirs; the
  addresses stay, the old introduction forwards.
- Data follows librotenado 0.15.0, from crates.io instead of its repository.
- **Short financial years:** a configuration that changes the start of the financial year ends the current one
  early (librotenado 0008); the review list names a change on a day that does not begin the new year.
- The review list names a reopening of something that is not a closing, and a closing reopened twice (librotenado
  0002).
- **Reopening a period:** the latest closing is reopened with a reason – in the terminal (`u`), the web app, the API
  and with `spezado reopen`; nothing is deleted, the reopening is an event of its own.
- **The balance sheet shows the result on equity:** the result of the financial year so far as the result for the
  year, earlier years as retained earnings – without a closing booking (librotenado 0004). It balances now, also
  as a filing.

## 0.3.0 – 2026-10-29

spezado in the menu bar, and for apps that install it themselves. Still a preview – the German mappings are drafts
until tax advisers have reviewed them.

- **`spezado tray`:** the server with an icon in the menu bar (macOS) or the system tray (Windows) – open the web
  app, pair a device, start at login, quit. Built into the releases for macOS and Windows; Linux keeps `serve`
  without GTK.
- **`spezado autostart on|off`:** starts spezado at login – a launch agent on macOS, the Run key on Windows, an XDG
  autostart entry on Linux.
- **For apps that start spezado themselves:** `serve` and `tray` create the workspace if there is none and take the
  pairing code from `SPEZADO_PAIRING_CODE` (at least 16 letters, digits, `-` or `_`) – in the environment rather
  than on the command line, where others on the device could read it.

## 0.2.0 – 2026-10-29

Books on several devices, and shared with other people: synchronised through a relay, end-to-end encrypted. Still a
preview – the German mappings are drafts until tax advisers have reviewed them. Data follows librotenado 0.6.

- **Synchronisation** between a person's own devices through a relay, end-to-end encrypted after librotenado 0006:
  `spezado relay`; starting it for books with a recovery code, inviting another device, joining, recovering;
  `spezado sync`, `y` and `v` in the terminal, the panel on the web app's overview, and every minute while
  `spezado serve` runs.
- **Sharing** books with other people – read, write or manage, set by the invitation –, **removing** a device or a
  person (the key of the books changes), and books a device may only read refuse to book.
- The **keys** of synchronised books in the key store of the system, else in the workspace.
- The **seller's details, customers and invoice numbers** in the books: they travel with them, and every device
  writes its own series of numbers (`RE-2026-B001`).
- **Other devices** reach `spezado serve --listen` over TLS, with a certificate of the workspace's own whose
  fingerprint the pairing link carries, also as a QR code.
- **Licences:** the description of the API (`api/openapi.json`) is MIT, so that clients generated from it can be
  licensed freely. The brand kit moved to its own repository, spezado/brand; `brand/` keeps copies of what spezado
  uses, the artwork under CC BY-ND 4.0, Domitian under OFL-1.1.

## 0.1.0 – 2026-10-29

The first release, a preview: a freelancer's year in the terminal and in the web app, in German and English. Not for
a real tax return yet – the German mappings (VAT return, Anlage EÜR, SKR 03/04) are drafts until tax advisers have
reviewed them. Data follows librotenado 0.4.1.

- **Books** for a person: cash basis or double entry, standard VAT or small business, SKR 03 or 04, the state whose
  public holidays move deadlines and the permanent extension of VAT returns – both can be changed from a date on;
  opening balances.
- **Booking** from a gross amount and a tax code or in lines; reversals; the journal, balances and ledgers.
- **Bank statements** read from camt.053, MT940 and CSV files, a bank account per IBAN, entries booked by hand or by
  a proposal – the payment booked before the statement came, the invoice it pays, or as the same party's last entry.
- **Documents**: e-invoices (XRechnung, ZUGFeRD) read into booking proposals, other documents filed; the originals
  kept by their hash.
- **Invoices** written as XRechnung 3.0 or as ZUGFeRD – a PDF/A-3 with the XRechnung attached – with the seller's
  details, customers and numbers in sequence – with VAT,
  exempt, under reverse charge (§ 13b UStG, services to businesses in other EU countries), intra-Community supplies,
  exports, or as a small business.
- **Reports**: the advance VAT returns with their deadlines, the Anlage EÜR 2025 and 2026, the HGB statements; CSV.
- **Review list** in words, **closing** periods, the **GoBD** data transfer (Z3).
- **Interfaces**: the terminal (keyboard and mouse), the command line, the open API (`spezado serve`, OpenAPI,
  Server-Sent Events, pairing per device) and the web app it serves.
