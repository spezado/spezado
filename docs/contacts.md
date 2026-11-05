---
title: Contacts
nav_order: 6.5
---

# Contacts

The customers and suppliers of the books – one contact may be both. Each has a **number**: the next free one from
10001 for customers and from 70001 for suppliers unless you give one; an address, e-mail and phone, the VAT
identification number and another tax number, a bank account, the reference it asks to be named on invoices (the
Leitweg-ID of a public authority), and **contact persons** with their own e-mail and phone.

Contacts are part of the books after [librotenado 0008](https://librotenado.dev/spec/0008-parties.html): each an
organisation of the books, its relation to the owner – customer, supplier, its number – an event of its own, and its
contact persons persons with the role `contact`. They travel with the books to every device. Two devices offline may
give the same number to two contacts; the review list names it.

| Where | How |
|---|---|
| Terminal | tab 9 *Contacts*: `k` new contact, `↵` change one |
| Web app | *Contacts* |
| API | `GET` `POST /api/v1/books/{books}/contacts` |

The customers the invoice form offers are the contacts that are customers; a customer added there is a contact, too.

## Documents name their contact

Reading an e-invoice offers the contact its other party may be – the seller of a purchase, the buyer of a sale –, the
most certain first: by VAT identification number, by another tax number, by the bank account the invoice asks to be
paid to, by name (ignoring case and the spaces between words). The first is chosen; choose another, none, or keep
the party as a new contact with what the invoice says of it. The document names the contact chosen.
