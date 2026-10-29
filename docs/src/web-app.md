# The web app

`spezado serve` serves the web app and the [API](api.md) on this device, at `http://127.0.0.1:8048` (another port with
`--port`). Nothing is loaded from elsewhere – no fonts, no scripts, no tracking.

```sh
spezado serve
# spezado serves at http://127.0.0.1:8048
# Pair a device with the code 6958d36774 – or open http://127.0.0.1:8048/#pair=6958d36774
```

## Other devices

To reach spezado from a phone, a tablet or an app in the same network, let it listen there – only over TLS:

```sh
spezado serve --listen 0.0.0.0:8443 --host laptop.local
```

It makes a certificate of the workspace's own for the host name and keeps it. The pairing link carries the
certificate's fingerprint and is printed as a QR code: an app checks the fingerprint, a browser asks once whether to
trust the certificate. A server with a domain can put a reverse proxy with a certificate of its own in front instead
([Synchronisation](sync.md), running a relay).

## Pairing

Every browser is paired once: open the address with `#pair=…` that `serve` shows, or enter the code on the page. The
code is valid for a quarter of an hour and for one device; a new start of `serve` shows a new one. The browser keeps
the token it receives; another browser needs a code of its own.

## The pages

| Page | |
|---|---|
| Overview | bank and cash, income, expenses and result of the year, the next advance VAT return and when it is due; the state and the permanent extension that move deadlines, changed from a date on; synchronisation – starting it, the devices, inviting one, synchronising now ([Synchronisation](sync.md)) |
| Journal | every booking, reversals |
| Bank | read statements, book entries or accept the proposals |
| Documents | read e-invoices into booking proposals, file other documents, download every original |
| Invoices | the seller's details, customers, invoices written as XRechnung or ZUGFeRD ([Invoices](invoices.md)) |
| Accounts | balances, ledgers, opening balances |
| Reports | the reports of the year, as CSV, too |
| Review | what needs review, in words |
| Closing | closing periods, the GoBD data transfer |

Without books, the web app sets up new ones – or brings them from another device, with an invitation or the recovery
code. While `serve` runs, it synchronises every minute.

`+ Buchen` / `+ Book` books from a gross amount or in lines. The language (German, English) and light or dark – or
like the system – are chosen at the top and kept in the browser. What another browser, the terminal or the command
line writes shows up at once.
