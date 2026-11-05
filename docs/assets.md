---
title: Fixed assets
nav_order: 6.6
---

# Fixed assets

What you buy to use for more than a year – a laptop, a car, office furniture – is not an expense at once: its cost is
spread over the years you use it. spezado keeps a **register of fixed assets** after
[librotenado 0011](https://librotenado.dev/spec/0011-assets.html):

1. **Book the purchase** on the asset account, as any booking – the laptop on 0420 (SKR 03) or 0650 (SKR 04), the VAT
   deducted as usual.
2. **Register the asset**: its name, the account, the day acquired, its cost without VAT, and how it is depreciated –
   on a **straight line** over its useful life in months (by month from the month acquired, § 7 Abs. 1 EStG), **at
   once** as a low-value asset of up to 800 euros (§ 6 Abs. 2 EStG), in a **pool** of the year for 250 to 1,000 euros,
   a fifth a year (§ 6 Abs. 2a EStG), or **not at all** – land, financial assets.
3. **Book the depreciation of the year** once, at its end: one booking, per asset its expense – depreciation of
   movable or intangible assets, of buildings, of vehicles, low-value assets, the pool – against the asset account.
   A year booked twice is refused.
4. **When it leaves** – sold, withdrawn, scrapped – record the day: its book value is booked out on the account of
   disposals. Book what it was sold for as a sale of its own, on the account of proceeds from fixed assets. If the
   depreciation of that year is booked already, reverse it first, record the disposal, and book it again – the register
   depreciates up to the month it leaves.

| Where | How |
|---|---|
| Terminal | tab *Accounts*: `a` new fixed asset, `f` book the depreciation of the year |
| Web app | *Assets* |
| API | `GET` `POST /api/v1/books/{books}/assets`, `POST /assets/{asset}/disposal`, `POST /depreciation` |

On a cash basis, the report **Anlage AVEÜR** (`de-aveuer`) shows the register by the groups of the form – land,
buildings, intangible assets, vehicles, office equipment, other movable assets, financial assets – and the pools; the
depreciation and the book values that went out reach the Anlage EÜR on their own lines. Which useful life applies is
yours to choose – the official tables of the BMF guide it; the German mappings are drafts until tax advisers have
reviewed them.
