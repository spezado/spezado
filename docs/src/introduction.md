# spezado

**Free accounting for everyone** – from private finances to freelancers, companies and non-profits. spezado keeps the
books of as many organisations as you like, runs on your own device – in the terminal, as a web app or on the command
line – and keeps working offline. It is free software (AGPL-3.0-or-later), with every feature included and no
telemetry.

This is the documentation of **version 0.3, a preview**: a freelancer's year in the terminal and in the web app, in
German and English. It is not yet for a real tax return – the German mappings (advance VAT return, Anlage EÜR,
SKR 03/04) are drafts until tax advisers have reviewed them.

## What 0.3 does

- **Books** for a person: cash basis or double entry, standard VAT or small business, SKR 03 or SKR 04, opening
  balances; the state whose public holidays move deadlines and the permanent extension of VAT returns.
- **Bookings** from a gross amount and a tax code or in lines; reversals instead of changes; journal, balances and
  ledgers.
- **Bank statements** from camt.053, MT940 and CSV files, matched to bookings, invoices and earlier payments.
- **Documents and e-invoices**: XRechnung and ZUGFeRD read into booking proposals, other files filed; every original
  kept by its hash.
- **Invoices** written as XRechnung or ZUGFeRD – with VAT, exempt, under reverse charge, to other EU countries, as
  exports or as a small business.
- **Reports**: the advance VAT returns with their deadlines, the Anlage EÜR, the HGB statements; closing periods and
  the GoBD data transfer.
- A **review list** in words: whatever breaks a rule of accounting is shown, never fixed silently.
- **Synchronisation** between a person's own devices through a relay, end-to-end encrypted, with a recovery code –
  shared with other people who read or book, keys in the key store of the system ([Synchronisation](sync.md)).
- **In the menu bar** with `spezado tray`, started at login if wished ([The web app](web-app.md)); the
  [desktop app](https://github.com/spezado/apps) installs and starts it by itself.

## How to read it

| If you want to … | read |
|---|---|
| try spezado | [Getting started](getting-started.md) |
| work in the terminal or the browser | [The terminal](terminal.md), [The web app](web-app.md) |
| script it | [The command line](command-line.md), [The API](api.md) |
| write invoices | [Invoices](invoices.md) |
| know how it is built and what comes next | [Design](design.md) |

The data follows the open protocol [librotenado](https://librotenado.dev), which any program may implement.
spezado is a project of [falkOSS](https://falkoss.org); the source is on
[GitHub](https://github.com/spezado/spezado). Statements marked **OPEN** are not decided or not verified yet.
