# The API

`spezado serve` offers an open HTTP API under `/api/v1` – the same one the web app uses. It is described in OpenAPI
3.1: [openapi.json](openapi.json), also written by `spezado openapi`.

## Pairing and tokens

Every client pairs once with the code `serve` shows and then sends its token with every request:

```sh
curl -s -X POST http://127.0.0.1:8048/api/v1/pair \
  -H 'Content-Type: application/json' -d '{"code": "6958d36774", "name": "script"}'
# {"token": "…", "device": "…"} – the token is shown once
curl -s http://127.0.0.1:8048/api/v1/books -H "Authorization: Bearer $TOKEN"
```

`GET /api/v1/devices` lists the devices paired, `DELETE /api/v1/devices/{device}` revokes one.

## What it offers

| Area | Endpoints |
|---|---|
| Books | `GET` `POST /books` – list, set up; `GET /books/{books}/overview`, `/journal`, `/accounts`, `/balances`, `/tax-codes` |
| Booking | `POST /books/{books}/bookings/simple`, `/bookings/lines`, `/reversals`; `GET` `POST /opening` |
| Bank | `POST /books/{books}/bank/statements`; `GET /bank/entries`; `POST /bank/entries/book`, `/bank/entries/accept`, `/bank/accept-all` |
| Documents | `GET` `POST /books/{books}/documents`; `GET /documents/{document}/original` |
| Invoices | `GET` `PUT /books/{books}/invoicing/profile`; `GET` `POST /customers`; `POST /invoices`; `GET /invoices/next-number` |
| Reports | `GET /books/{books}/reports`, `/reports/{report}`; `GET` `PUT /deadlines`; `GET /review` |
| Closing | `POST /books/{books}/closings`; `GET /gobd` |
| Changes | `GET /events` – Server-Sent Events: `changed` with the books whenever something was written |

Amounts are written with their currency, the value as a decimal string with a point –
`{"value": "-12.40", "currency": "EUR"}`; amounts sent are decimal strings (`"1234.50"`), dates `YYYY-MM-DD`. Errors
come as `{"error": "…"}`, in English, with a status of 400 (a request that cannot be read), 401 (no valid token), 404
or 422 (refused, e.g. a booking that does not balance). A browser, which cannot set headers for an event
stream, passes its token as the query `token`.
