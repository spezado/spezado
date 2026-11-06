---
title: The command line
nav_order: 5
---

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
| `reopen` | reopens the latest closing, with `--reason`; nothing is deleted |
| `filing` | lists the filings; `prepare`, `submit [--test]`, `refresh` ([Filings](filings.md)) |
| `adapter` | shows or sets the adapter that submits filings in a country, the token from `SPEZADO_ADAPTER_TOKEN` |
| `export`, `import` | the events of the books as JSON Lines, out and in |
| `sync` | synchronises books with other devices and people: `start`, `invite --rights`, `join`, `recover`, `devices`, `remove` ([Synchronisation](sync.md)) |
| `relay` | runs a relay |
| `tui`, `serve` | the terminal; the web app and API – `serve --listen 0.0.0.0:8443` for other devices, over TLS ([The web app](web-app.md)) |
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

The reports for German books are `de-ustva` (advance VAT return), `de-ust` (annual VAT return, over a calendar
year) and `de-zm` (recapitulative statement, per customer); `de-euer` (Anlage EÜR) and `de-aveuer` (Anlage AVEÜR, the fixed
assets) on a cash basis, and the balance sheet and profit and loss account after the HGB with double entry – for a
corporation also the shortened balance sheets `de-hgb-266-small` and `de-hgb-266-micro` (§ 266 Abs. 1 HGB). A
balance sheet notes per position how much is due within a year and after more than a year (§ 268 Abs. 4 and 5 HGB),
from the remaining terms of the SKR accounts, and – where bookings count in the commercial or the tax balance sheet
only (`basis` in the API) – the reconciliation to the tax balance sheet. `--year-start-day` lets a financial year
begin in the middle of a month.

The taxes on profit over a financial year: `de-kst` (corporation tax and solidarity surcharge, for corporations)
and `de-gewst` (trade tax, where the configuration has the multiplier of the municipality: `--trade-tax-multiplier`
when setting up, `spezado trade-tax 410 --from 2026-01-01` later; `spezado trade-tax --from …` without a number for
a freelancer). Losses carried forward, the reduction for business land and the size of holdings are not asked for
yet (librotenado 0012, OPEN).

For a person running a business, `de-est` is the income tax over a calendar year: the profit of the books with what
the person states beside them – `spezado income-tax --year 2026 --other-income 12000.00 --deductions 4000.00
--losses 0 --joint false --church 9`; without options it shows what is stated. It stays on this device. What the books
do not hold of the taxes on profit – losses carried forward, a reduction for land, the share held in corporations –
is stated with `spezado profit-tax --year 2026 --income-losses … --trade-losses … --holding at_least_15`.
`de-anlage-g` and `de-anlage-s` give the lines of the profit in the income tax return. Hidden profit distributions,
tax groups and the interest barrier are stated with `profit-tax` as well (`--hidden-distributions`, `--group member
--group-compensation …`, `--interest-carryforward …`); children, single parents, extraordinary income, partnership
shares and capital income with `income-tax` (`--child-allowances 10 --child-benefit 3108.00`, `--single-parent-children
1`, `--extraordinary …`, `--partnership-income …`, `--capital-income …`). A partnership opens one account per partner
and kind of capital with the attributes `partner`, `liability` (`unlimited`, `limited`) and `capital` (`FK`, `VK`, …);
the report `de-ebilanz-kke` shows how they developed.

## Payroll

An employer describes each employment once – the employee by name, the tax features (ELStAM) and how the employee
is insured – and computes a payslip per month by librotenado's rules (0013):

```sh
spezado employment add --name "Erika Muster" --start 2026-01-01 --class 1 --church rk \
  --insurer 87654321 --groups 1111 --person-group 101 --additional-rate 2.9 --u1 1.5 --u2 0.4
spezado employment
spezado payslip --employment <id> --month 2026-01 --wage 4000.00
spezado payslip --employment <id> --month 2026-02 --wage 4000.00 --one-off 2000.00 --record
spezado contributions --insurer 87654321 --from 2026-02-01 --to 2026-02-28
```

A minor employment: `--flat --groups 6500 --person-group 109` with the Minijob-Zentrale as insurer, in a private
household with `--household`; a short-term one: `--short-term --groups 0000 --person-group 110 --end …`. A payslip
recorded wrongly is corrected with `spezado payslip --corrects <payslip> --wage … --record`; the difference is
booked on the day given with `--paid`. `spezado wage-tax-certificate --employment <id> --year 2026` shows the lines
of the Lohnsteuerbescheinigung. Beitragsnachweise and notifications to the social insurance are transmitted only from
approved programs (§ 95b SGB IV): enter what `spezado contributions` shows in the SV-Meldeportal. `--record`
records the payslip and books it on the day it is paid, the last of the month. The Lohnsteuer-Anmeldung is the
report `de-lsta` of the period; the Beitragsnachweis is per insurer and month.
