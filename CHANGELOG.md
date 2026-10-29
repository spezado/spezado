# Changelog

## Unreleased

- **`spezado serve` for apps that start it themselves:** it creates the workspace if there is none, and takes the
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
