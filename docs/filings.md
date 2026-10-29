---
title: Filings
nav_order: 7
---

# Filings

A filing is what goes to an authority: in Germany the advance VAT return (`de-ustva`) and the Anlage EÜR (`de-euer`).
spezado derives it from the books, records it, and submits it through an **adapter** – a service of its own per
country that talks to the authority, after
[librotenado 0007](https://librotenado.dev/spec/0007-filings.html). spezado itself never talks to ELSTER.

**OPEN:** there is no adapter for ELSTER yet. It needs ERiC, the library of the German tax administration, which is
handed out to registered developers only; falkOSS registers once it is founded. Until then filings are prepared and
recorded, and can be tried with any adapter that follows librotenado 0007.

## Preparing

A filing is prepared from a report and its period: its fields as the books give them now, and the state of the books
they were derived from. It is never changed afterwards.

- Asked again while nothing changed, spezado returns the same filing.
- When the books change so that the fields would differ – a booking in the period, a reversal –, the filing is
  **outdated**: it is not submitted; prepare it again.
- A filing prepared after one was submitted or accepted is a **correction** of it (in German a *berichtigte
  Voranmeldung*).
- A VAT return with nothing to report is a filing too.

## Submitting

The adapter is set per country: its address – `https://…`, or `http://127.0.0.1:…` on this device – and the token
spezado authenticates itself with. The token goes into the key store of the system where there is one.

A **test submission** goes to the adapter marked as a test, which the authority does not process; nothing is recorded
in the books. A real one moves the filing on: *submitted*, then *accepted* or *rejected* as the authority answers.
What it returns – a protocol of the transfer, a notice – becomes a document of the books, with its original. A
rejected filing is submitted again, or corrected.

| | Terminal | Web app | Command line |
|---|---|---|---|
| prepare | `p` on a report's figures | *Prepare as a filing* on a report | `spezado filing prepare de-ustva --from … --to …` |
| list | the filings after the reports | *Filings* under the reports | `spezado filing` |
| submit | `↵` on a filing: a test or for real | *Submit as a test*, *Submit* | `spezado filing submit <id> [--test]` |
| ask where it stands | `↵` on a submitted filing | *Ask where it stands* | `spezado filing refresh <id>` |
| set the adapter | – | *Filings*, *change* | `SPEZADO_ADAPTER_TOKEN=… spezado adapter DE <url>` |
