---
title: Getting started
nav_order: 2
---

# Getting started

## Download

Every release has an archive per platform – Linux (x86_64, arm64), macOS (arm64) and Windows (x86_64) – with a
SHA-256 checksum: [releases](https://github.com/spezado/spezado/releases). Unpack it; the archive holds the program
`spezado` (`spezado.exe`), the licences and the changelog. The web app is built into the program.

```sh
shasum -a 256 -c spezado-0.3.0-aarch64-apple-darwin.tar.gz.sha256
tar -xzf spezado-0.3.0-aarch64-apple-darwin.tar.gz
./spezado-0.3.0-aarch64-apple-darwin/spezado tui
```

The binaries are not signed: macOS and Windows may ask before the first start.

## Build it yourself

spezado is written in Rust; the toolchain is picked up from `rust-toolchain.toml` by [rustup](https://rustup.rs).
The web app needs [Node.js](https://nodejs.org) and [pnpm](https://pnpm.io) to be built – without it, `serve` offers
the API alone.

```sh
git clone https://github.com/spezado/spezado
cd spezado
(cd web && pnpm install && pnpm build)   # the web app, embedded into the binary
cargo run --release -- tui               # the terminal
cargo run --release -- serve             # or the web app: open the address it shows
```

## The workspace

A workspace is where this device keeps its books: a SQLite database and the originals of your documents. It lives in
spezado's folder in your data directory, or wherever `--dir` or the environment variable `SPEZADO_DIR` say. One
workspace holds as many sets of books as you like; `--books` picks one where there are several.

## The first books

Start the terminal (`spezado tui`): without books it asks for them – name, first day, how profit is determined (cash
basis or double entry), the VAT scheme with basis and period of the advance returns, the chart of accounts (SKR 03 or
SKR 04), the permanent extension and the state whose public holidays move deadlines. The web app asks the same on its
first start; on the command line it is `spezado set-up` ([The command line](command-line.md)).

Then, in this order:

1. **Opening balances** – what the bank account and the cash held on the first day ([The terminal](terminal.md):
   `o` on *Accounts*), so that bank statements close at the balance of the books.
2. **Bank statements** – read the files your bank gives you; spezado proposes a booking for every entry.
3. **Documents** – read e-invoices into booking proposals, file everything else with the booking it backs.
4. **Reports** – the advance VAT returns when they fall due, the Anlage EÜR at the end of the year.
5. **Review** – look at the review list now and then; it names what does not fit, in words.
