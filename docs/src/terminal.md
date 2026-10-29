# The terminal

`spezado tui` starts the terminal interface. It works with the keyboard and the mouse: every key shown at the bottom
is also a button, rows can be clicked and scrolled. With the mouse on, selecting text needs the modifier key of
your terminal; `m` switches the mouse off.

## Moving around

| Key | |
|---|---|
| `1` … `8`, `Tab`, `Shift-Tab` | the tabs: Overview, Journal, Bank, Documents, Accounts, Reports, Review, Closing |
| `↑` `↓` | the rows of a tab |
| `↵` | open the row: a bank entry to book, an account's ledger, a report |
| `[` `]` | the year before, the year after |
| `s` | German or English |
| `m` | mouse on or off |
| `q`, `Ctrl-C` | quit |

In a form, `↵`, `Tab` or `↓` go to the next field and `↵` on the last one confirms; `Shift-Tab` or `↑` go back,
`←` `→` change a choice and `Esc` cancels. An account field searches as you type – by number or name – and `↵`
takes the account picked with `↑` `↓`.
Amounts and dates are written as in the language shown: `1.234,50` and `29.10.2026` in German, `1,234.50` and
`2026-10-29` in English. Accounts are given by their number in your chart (`4930`) – or, for a bank account, by its
IBAN.

## The tabs

| Tab | Keys | |
|---|---|---|
| Overview | `n` new booking, `i` read a document, `d` deadlines, `y` synchronise (or set it up), `v` invite a device | bank and cash, income, expenses and result of the year, the next advance VAT return and when it is due, open entries, review, the date closed up to, and the relay the books are synchronised through |
| Journal | `n` new booking, `z` booking in lines, `r` reverse | every booking; a reversal refers to what it cancels – nothing is changed or deleted |
| Bank | `i` read a statement, `↵` book, `a` book all proposals, `f` fetch | the entries of your statements not booked yet, each with a proposal where spezado found one |
| Documents | `i` read, `n` new booking, `r` write an invoice, `k` new customer, `p` seller | documents with their originals |
| Accounts | `↵` ledger, `o` opening balances | the balance of every account |
| Reports | `↵` open, then `e` export as CSV | the reports that apply to the year |
| Review | | what needs review, in words |
| Closing | `c` close a period, `g` GoBD data transfer | |

**A new booking** (`n`) is date, text, account, gross amount, tax code and payment from the bank or the cash:
spezado books the net amount, the VAT and the payment. **A booking in lines** (`z`) takes lines of `account=amount[@code]`,
separated by spaces, positive for debit and negative for credit:
`4930=10,00@de-in-19 1576=1,90 1200=-11,90`.

**Deadlines** (`d`): the state whose public holidays move deadlines (§ 108 Abs. 3 AO) and the permanent extension
of advance VAT returns (§ 46 UStDV), from a date on.

**Bank proposals** come in this order: a payment booked before the statement came (it is booked again on the
statement's bank account, naming the entry), the invoice the entry pays, and the same party's last entry. A proposal
is only a proposal: `↵` shows it to confirm or change.

**Fetching from the bank** (`f`) needs a registration as a FinTS product, which spezado does not have yet – read the
statement files your bank gives you instead.

**Opening balances** (`o`): per account `account=amount`, holdings positive and debts negative – `1200=3.160,50
1000=200`. They are booked once, against the opening balance sheet, and carry on from year to year by themselves.
