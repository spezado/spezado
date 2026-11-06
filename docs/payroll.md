---
title: Payroll and taxes
nav_order: 6.7
---

# Payroll and taxes

spezado computes the wages of employees after [librotenado 0013](https://librotenado.dev/spec/0013-payroll.html) and
the taxes on profit and income tax after [0012](https://librotenado.dev/spec/0012-taxes-on-profit.html). The rules –
every limit and rate with its provision, the Programmablaufplan of the BMF, cases to check – can be read and tried in
the [rulebook](https://librotenado.dev/rules/). All of it is a draft until tax advisers have reviewed it.

## Employees and payslips

- **Web app**, page *Payroll*: a new employee with the features for wage tax (class, child allowances, church) and
  social insurance (health insurer, contribution groups, person group, additional rate); a payslip of a month –
  computed first, then recorded and booked –; the payslips of the year, each to be corrected; the Beitragsnachweis of
  a month per insurer and the Lohnsteuerbescheinigung of a year.
- **Terminal**, tab `0` *Payroll*: `b` a new employee, `a` a payslip of a month, recorded and booked, `↵` its figures.
- **Command line**: `spezado employment add`, `spezado payslip … [--record] [--corrects …]`,
  `spezado contributions`, `spezado wage-tax-certificate` ([Command line](command-line.md#payroll)).

A payslip is booked on Gehälter or Aushilfslöhne, gesetzliche soziale Aufwendungen and the liabilities for wage tax,
social security and net pay. A correction books the difference. Minor and short-term employment, private households,
trainees with low pay, company pensions and the Aktivrente are set on the employee.

**Beitragsnachweise and notifications to the social insurance are not transmitted.** The law allows that only from
programs approved by the GKV-Spitzenverband (§ 95b SGB IV); enter what spezado shows in the SV-Meldeportal of the
insurers. The Lohnsteuer-Anmeldung and the certificates go to ELSTER once the adapter exists ([Filings](filings.md)).

## What is due

The overview of the web app lists what is due next, `spezado obligations --year 2026` all of a year, the reports tab
of the terminal the same by period:

| What | When |
|---|---|
| Umsatzsteuer-Voranmeldung, Lohnsteuer-Anmeldung | the 10th after the period – a month, a quarter or the year, for wage tax by the wage tax of the year before (§ 41a EStG) |
| Zusammenfassende Meldung | the 25th after the period |
| Beitragsnachweis | the beginning of the fifth last banking day of the month; the contributions on the third last |
| Lohnsteuerbescheinigungen | the last day of February |
| Annual returns – VAT, EÜR, income, corporation and trade tax, balance sheet | 31 July of the year after (§ 149 Abs. 2 AO); with a tax adviser later |

A deadline on a weekend or a public holiday of the state set in the overview moves to the next working day.

## Taxes beside the books

The web app's page *Taxes*, `spezado profit-tax` and `spezado income-tax` take what the books do not hold, per year,
kept on this device: losses carried forward, hidden profit distributions and contributions, a tax group, the interest
barrier; other income, children and child benefit, capital income, extraordinary income, shares of partnerships. The
reports `de-kst`, `de-gewst`, `de-est`, `de-anlage-g` and `de-anlage-s` compute with them.

A partnership opens the capital accounts of each partner on the page *Taxes* – fixed and variable capital, a private
account, for a limited partner the account of losses –; the report `de-ebilanz-kke` shows how they developed, as the
E-Bilanz asks.
