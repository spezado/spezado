# Contributing to spezado

Thank you for helping. spezado is free accounting for everyone; its data follows the open protocol
[librotenado](https://github.com/librotenado/librotenado).

## How

1. Open an issue first for anything larger than a fix.
2. Fork, change, and open a pull request. CI checks format, lints, tests and licences.
3. Sign off every commit (`git commit -s`): with it you confirm the
   [Developer Certificate of Origin](https://developercertificate.org). There is no CLA.

## Setting up

spezado builds against the protocol library [librotenado](https://github.com/librotenado/librotenado), which Cargo
fetches from its repository (`Cargo.toml`). To work on both at once, point the dependency at a local checkout with a
`[patch]` section in your own `.cargo/config.toml` – never commit it.

```sh
cargo fmt --check
cargo clippy --all-targets    # no warnings
cargo test
cargo run -- --dir /tmp/spezado init
cargo run -- --dir /tmp/spezado set-up --name "Ada" --from 2026-01-01 --accounting de-cash-basis --vat de-regular --chart de-skr03
cargo run -- --dir /tmp/spezado book --date 2026-02-10 --text "Paper" 4930=12.40 1200=-12.40
```

The web app in `web/` needs Node 26 and pnpm:

```sh
cd web && pnpm install && pnpm api && pnpm check && pnpm build   # then cargo build embeds web/dist
pnpm dev                                                         # while `cargo run -- serve` runs
```

## Where things go

| Crate | What belongs there |
|---|---|
| `spezado-core` | the books: commands checked into events, queries, the review list – pure, no I/O |
| `spezado-law`, `spezado-law-de` | the interface of law modules and the German one – facts in, what follows out |
| `spezado-store` | storage: events in SQLite, originals as files |
| `spezado-app` | the workspace every interface works through: stamping and storing events, workflows |
| `spezado-tui`, `spezado-api`, `spezado` | the terminal interface, the API, the command line – thin, through `spezado-app` only |

What other implementations need to read and write the same way – events, fields, review rules – belongs in the
protocol, librotenado, not here.

## Rules

- **Licence:** AGPL-3.0-or-later, an SPDX header in every source file; files that cannot carry one are listed in
  `REUSE.toml`. Code that belongs in the protocol goes to librotenado (MIT) – written there, never copied from here.
- **Everything included:** no feature behind a payment, licence key or unlock; law modules are free software only.
- **No telemetry:** only connections that are technically needed or that users start themselves.
- **Events are the truth:** never change or delete a booking – reverse it. What does not fit goes to the review list.
- **Money is never a float.**
- **Sources:** legal and tax rules need a source; what is not verified is marked **OPEN**.
- English in code, docs and commits.
