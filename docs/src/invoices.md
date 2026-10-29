# Invoices

spezado writes invoices as **XRechnung 3.0** – an XML file after EN 16931 – or as **ZUGFeRD** – a PDF to read with the
same XRechnung attached for machines (profile XRECHNUNG). Since 2025 businesses in Germany must be able to receive
e-invoices ([BMF](https://www.bundesfinanzministerium.de/Content/DE/FAQ/e-rechnung.html)); both formats are
e-invoices. The file written is the original: it is kept with the invoice and can be downloaded again.

## Before the first invoice

- **The seller** – your name, address, contact with phone and e-mail, VAT identification number and/or tax number,
  the IBAN to pay to, the days until payment is due and the prefix of the numbers (terminal: `p` on *Documents*).
- **A customer** – name, address, the e-mail invoices go to, the VAT identification number for supplies to businesses
  abroad or under reverse charge, and the reference a public authority asked for (its Leitweg-ID) (terminal: `k`).

## Writing one

An invoice has a customer, a date, the day of the supply, lines – what, how much, the unit (hours, days, pieces), the
net price and a tax code –, a note and the file to write. Numbers follow in sequence per year: `RE-2026-001`,
`RE-2026-002`, … On a cash basis the invoice is booked when it is paid – the bank proposes it for the entry that
pays it –; with double entry the receivable is booked when the invoice is issued.

| Tax code | Category (EN 16931) | What the invoice states |
|---|---|---|
| `de-out-19`, `de-out-7` | S – 19 % or 7 % | |
| `de-out-0` | Z – 0 % (§ 12 Abs. 3 UStG) | |
| `de-out-exempt` | E – exempt (§ 4 UStG) | the reason you give – „Steuerfrei nach § 4 Nr. 14 UStG.“ –, a general one otherwise |
| `de-out-13b` | AE – reverse charge at home (§ 13b UStG) | „Steuerschuldnerschaft des Leistungsempfängers“ |
| `de-out-eu-service` | AE – a service to a business in another EU country (§ 3a Abs. 2 UStG) | „Steuerschuldnerschaft des Leistungsempfängers“ |
| `de-out-ig-supply` | K – intra-Community supply | „Steuerfreie innergemeinschaftliche Lieferung“ |
| `de-out-export` | G – export | „Steuerfreie Ausfuhrlieferung“ |
| none – small business | E – § 19 UStG | „Gemäß § 19 UStG wird keine Umsatzsteuer berechnet.“ |

The statements are those § 14 Abs. 4 Nr. 8 and § 14a UStG ask for
([§ 14a UStG](https://www.gesetze-im-internet.de/ustg_1980/__14a.html)). spezado checks before writing what they
need: a customer in Germany for § 13b, a customer in another country for EU services and intra-Community supplies,
and the VAT identification numbers of both sides where they must be on the invoice.

## XRechnung and ZUGFeRD

Both carry the same data and are checked against the rules of XRechnung before they are written.

- **XRechnung** (`RE-2026-001.xml`, UBL): XML only, as the German public administration receives invoices
  ([XRechnung](https://xeinkauf.de/xrechnung/)).
- **ZUGFeRD** (`RE-2026-001.pdf`) is a PDF/A-3b for people – set in Domitian, with the address in the window of a
  DIN 5008 envelope – with the XRechnung in CII attached as `xrechnung.xml`. ZUGFeRD invoices written by spezado pass
  [veraPDF](https://verapdf.org) (PDF/A-3b) and the [Mustang](https://www.mustangproject.org) validator (PDF/A-3b,
  EN 16931 and XRechnung rules).

Without a VAT identification number the seller's tax number is also its identifier (BT-29): EN 16931 requires one of
BT-29, BT-30 or BT-31 (rule [BR-CO-26](https://docs.peppol.eu/poac/eu/pint-eu/trn-invoice/rule/BR-CO-26/)).

The seller's details and the customers are part of the books – the owner's contact and invoicing settings, the
customers as organisations – and travel with them to every device. Each device writes invoice numbers in a series of
its own, so that two devices offline never write the same number: the first `RE-2026-001`, the next ones
`RE-2026-B001`, `RE-2026-C001` (librotenado 0008). Should two devices ever have taken the same series, the review list
says so.
