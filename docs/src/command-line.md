# The command line

Everything the terminal does with books is open to scripts as well. Every command takes `--dir` (the workspace) and,
where the workspace holds several sets of books, `--books`; `spezado help <command>` shows its options.

| Command | |
|---|---|
| `init` | creates a workspace for this device |
| `books` | lists the books of the workspace |
| `set-up` | sets up books for a person or an organisation |
| `book` | records a booking from lines |
| `reverse` | reverses a booking by the event that recorded it |
| `journal`, `balance` | the journal, the balance of every account |
| `report` | a report for a period, or – without one – the reports that apply |
| `check` | what needs review |
| `close` | closes an owner's books up to a date |
| `export`, `import` | the events of the books as JSON Lines, out and in |
| `sync` | synchronises books with other devices: `start`, `invite`, `join`, `recover`, `devices` ([Synchronisation](sync.md)) |
| `relay` | runs a relay |
| `tui`, `serve` | the terminal, the web app and API |
| `openapi` | the description of the API |

## A freelancer's quarter

```sh
spezado init
spezado set-up --name "Ada" --from 2026-01-01 --accounting de-cash-basis \
  --vat de-regular --vat-basis cash --vat-period quarter --chart de-skr03 --region DE-BE
spezado book --date 2026-02-10 --text "Invoice 1" 1200=1190 8400=-1000@de-out-19 1776=-190@de-out-19
spezado book --date 2026-02-12 --text "Paper" 4930=10@de-in-19 1576=1.90@de-in-19 1200=-11.90
spezado journal
spezado report de-ustva --from 2026-01-01 --to 2026-03-31
spezado check
```

A line of `book` is an account – its number in the books' chart, or its UUID –, `=`, the amount with a decimal point,
positive for debit and negative for credit, and optionally `@` and a tax code; the lines must balance.

`set-up` without `--legal-form` sets up a person's books; with one (`de-gmbh`, `de-ev`, …) an organisation's.
`--accounting` is `de-cash-basis` or `de-double-entry`, `--vat` `de-regular` or `de-small-business`, `--chart`
`de-skr03` or `de-skr04`; `--vat-extended` and `--region` (`DE-BY`, `DE-NW`, …) move the deadlines.

The reports for German books are `de-ustva` (advance VAT return) and `de-euer` (Anlage EÜR) on a cash basis, and
the balance sheet and profit and loss account after the HGB with double entry.
