<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="brand/svg/spezado-lockup-on-dark.svg">
    <img src="brand/svg/spezado-lockup-on-light.svg" alt="spezado" width="420">
  </picture>
</p>

# spezado

**Free accounting for everyone** – from private finances to freelancers, companies and non-profits.
<sub>a [falkOSS](https://falkoss.org) project</sub>

spezado keeps the books of as many organisations as you like – each person has one account and works in every
organisation they have access to. It runs on its own as an app, in the terminal or behind a server for larger
installations, and keeps working offline: data is local first and synchronised end-to-end encrypted.

> **Status:** version 0.1.0, a preview – a freelancer's year in the terminal and in the web app, in German and
> English: bank statements, documents and e-invoices, invoices written as XRechnung or ZUGFeRD, the advance VAT returns
> and the Anlage EÜR. Not yet for a real tax return – the German mappings are drafts until tax advisers have reviewed
> them.
> The documentation is at [spezado.dev](https://spezado.dev), the design in [docs/spec.md](docs/spec.md); data
> follows the open protocol [librotenado](https://github.com/librotenado/librotenado).

## Trying it out

Download the archive for your platform from the [releases](https://github.com/spezado/spezado/releases), unpack it and
run `spezado tui` for the terminal interface or `spezado serve` for the web app – or build it yourself:

```sh
cargo run -- tui                       # the terminal interface: set up books, book, read e-invoices, report
```

or as a web app and API on this device (the API is described in `api/openapi.json`):

```sh
(cd web && pnpm install && pnpm build) # the web app, embedded into the binary
cargo run -- serve                     # open the address it shows: it pairs this browser
```

or on the command line:

```sh
cargo run -- init
cargo run -- set-up --name "Ada" --from 2026-01-01 --accounting de-cash-basis --vat de-regular --chart de-skr03
cargo run -- book --date 2026-02-10 --text "Invoice 1" 1200=1190 8400=-1000@de-out-19 1776=-190@de-out-19
cargo run -- journal
cargo run -- report de-ustva --from 2026-01-01 --to 2026-03-31
```

The workspace lives in spezado's folder in your data directory, or wherever `--dir` or `SPEZADO_DIR` say.

## Planned

- **Bookkeeping** with multiple currencies from day one, bank connection, automatic recognition of receipts and
  e-invoices, writing e-invoices, payroll.
- **Law as modules** – starting with Germany, then Austria and Switzerland and further European countries; filings to
  tax authorities through adapters (ELSTER first).
- **One core, many front ends** – a Rust core with a command line and a terminal interface, an open API with a web
  app, desktop and mobile apps ([spezado/apps](https://github.com/spezado/apps)); relays and a server for teams.

## Same rules as falkOSS

- **Free licences** – AGPL-3.0-or-later; the protocol library and the apps under MIT.
- **Everything included** – one edition, every feature free; modules are never proprietary.
- **No telemetry** – nothing phones home.

## Licence

[GNU Affero General Public License v3.0 or later](LICENSE), except the description of the API, `api/openapi.json`
(MIT), and the copies of the brand in `brand/` – the artwork under CC BY-ND 4.0, Domitian under OFL-1.1, from
[spezado/brand](https://github.com/spezado/brand); spezado's name and brand belong to falkOSS. Contributions with a
[DCO](https://developercertificate.org) sign-off (`git commit -s`), no CLA – see [CONTRIBUTING.md](CONTRIBUTING.md).

© 2026 falkOSS – Falcon Lake Open Source Software gUG (haftungsbeschränkt) i. G. and the spezado contributors
