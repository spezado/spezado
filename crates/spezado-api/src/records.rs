// SPDX-License-Identifier: AGPL-3.0-or-later
//! The routes of documents, the bank and the data transfer: files read in, proposals, entries booked.

use std::{
    collections::BTreeMap,
    io::Write,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::header,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use librotenado::{AccountId, Booking, BooksId, Document, DocumentId, EntryReference, InvoiceSide, PartyId, Payload};
use serde::Deserialize;
use spezado_app::{OpenEntry, Source, Workspace, today};
use spezado_core::Command;
use utoipa::IntoParams;

use crate::{Api, ApiError, dto};

/// How long a document read waits to be recorded.
const WAITING: Duration = Duration::from_secs(60 * 60);

/// Documents read and not recorded yet, by the identity handed out.
pub type Pending = BTreeMap<String, (BooksId, Document, Option<Booking>, Instant)>;

/// The routes of documents, the bank and the data transfer.
pub fn routes() -> Router<Api> {
    Router::new()
        .route("/books/{books}/documents", get(documents).post(read_document))
        .route("/books/{books}/documents/pending/{pending}", post(record))
        .route("/books/{books}/documents/{document}/original", get(original))
        .route("/books/{books}/bank/entries", get(open_entries))
        .route("/books/{books}/bank/statements", post(import))
        .route("/books/{books}/bank/entries/book", post(book_entry))
        .route("/books/{books}/bank/entries/accept", post(accept))
        .route("/books/{books}/bank/accept-all", post(accept_all))
        .route("/books/{books}/bank/fetch", post(fetch))
        .route("/books/{books}/gobd", get(gobd))
}

fn books_id(text: &str) -> Result<BooksId, ApiError> {
    text.parse().map_err(|_| ApiError::bad(format!("not the identity of books: {text:?}")))
}

fn owner(workspace: &Workspace, books: &BooksId) -> Result<PartyId, ApiError> {
    let owners = workspace.get(books)?.events().owners();
    owners.first().map(|o| (*o).clone()).ok_or_else(|| ApiError::bad("the books have no owner"))
}

fn names(
    workspace: &Workspace,
    books: &BooksId,
    owner: &PartyId,
) -> Result<BTreeMap<AccountId, (Option<String>, String)>, ApiError> {
    Ok(workspace
        .account_choices(books, owner, today(), "de")?
        .into_iter()
        .map(|c| (c.id, (c.number, c.name)))
        .collect())
}

fn booking_view(booking: &Booking, names: &BTreeMap<AccountId, (Option<String>, String)>) -> dto::BookingView {
    dto::BookingView {
        date: booking.date.to_string(),
        text: booking.text.clone(),
        postings: booking
            .postings
            .iter()
            .map(|p| {
                let (number, name) = names.get(&p.account).cloned().unwrap_or((None, p.account.to_string()));
                dto::PostingView {
                    account: p.account.to_string(),
                    number,
                    name,
                    amount: dto::Amount::from(&p.amount),
                    tax: p.tax.as_ref().map(ToString::to_string),
                }
            })
            .collect(),
    }
}

fn document_view(document: &Document, booked: bool) -> dto::DocumentView {
    let kind = serde_json::to_value(document.kind).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
    dto::DocumentView {
        id: document.id.to_string(),
        kind,
        number: document.number.clone(),
        date: document.date.to_string(),
        format: document.original.format.clone(),
        media_type: document.original.media_type.clone(),
        name: document.original.name.clone(),
        size: document.original.size,
        due: document.invoice.as_ref().map(|i| dto::Amount::from(&i.totals.due)),
        booked,
    }
}

/// The documents recorded, newest first.
#[utoipa::path(get, path = "/api/v1/books/{books}/documents", params(("books" = String, Path)),
    responses((status = 200, body = Vec<dto::DocumentView>)))]
async fn documents(
    State(api): State<Api>,
    Path(books): Path<String>,
) -> Result<Json<Vec<dto::DocumentView>>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let based: std::collections::BTreeSet<DocumentId> = workspace
            .get(&books)?
            .events()
            .ordered()
            .into_iter()
            .flat_map(|(_, event)| match &event.payload {
                Payload::BookingRecorded { booking } => booking.documents.clone(),
                _ => Vec::new(),
            })
            .collect();
        let mut list: Vec<dto::DocumentView> =
            workspace.documents(&books)?.iter().map(|(_, d)| document_view(d, based.contains(&d.id))).collect();
        list.reverse();
        Ok(Json(list))
    })
    .await
}

/// How a document is read.
#[derive(Debug, Deserialize, IntoParams)]
pub struct ReadQuery {
    /// The file name.
    name: Option<String>,
    /// `purchase` (default) or `sale`.
    side: Option<String>,
    /// `bank` (default) or `cash`: where it was paid from or to.
    payment: Option<String>,
}

/// Reads a file – an e-invoice (XRechnung, ZUGFeRD), a PDF, a photo – as a document, with a proposal for an e-invoice.
/// Nothing is recorded until it is named in `documents/pending/{pending}` within the hour.
#[utoipa::path(post, path = "/api/v1/books/{books}/documents", params(("books" = String, Path), ReadQuery),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 200, body = dto::ReadDocument), (status = 422, body = dto::Problem)))]
async fn read_document(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<ReadQuery>,
    bytes: Bytes,
) -> Result<Json<dto::ReadDocument>, ApiError> {
    let pending = api.pending.clone();
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let standard = workspace.standard_accounts(&books, &owner, today())?;
        let payment = if query.payment.as_deref() == Some("cash") { standard.cash } else { standard.bank };
        let side = if query.side.as_deref() == Some("sale") { InvoiceSide::Sale } else { InvoiceSide::Purchase };
        let read = workspace.read_document(&books, &owner, &bytes, query.name.as_deref(), side, &payment)?;
        let names = names(workspace, &books, &owner)?;
        let id = spezado_app::new_id::<DocumentId>().to_string();
        let result = dto::ReadDocument {
            pending: id.clone(),
            document: document_view(&read.document, false),
            proposal: read.proposal.as_ref().map(|b| booking_view(b, &names)),
            not_an_invoice: read.not_an_invoice,
        };
        let mut waiting = pending.lock().expect("the documents waiting");
        waiting.retain(|_, entry| entry.3.elapsed() < WAITING);
        waiting.insert(id, (books, read.document, read.proposal, Instant::now()));
        Ok(Json(result))
    })
    .await
}

/// Records a document read – alone, or with its proposal, dated and worded as given.
#[utoipa::path(post, path = "/api/v1/books/{books}/documents/pending/{pending}",
    params(("books" = String, Path), ("pending" = String, Path)), request_body = dto::Record,
    responses((status = 200, body = dto::Written), (status = 404, body = dto::Problem), (status = 422, body = dto::Problem)))]
async fn record(
    State(api): State<Api>,
    Path((books, pending)): Path<(String, String)>,
    Json(request): Json<dto::Record>,
) -> Result<Json<dto::Written>, ApiError> {
    let id = books.clone();
    let waiting = api.pending.clone();
    let written = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let (of, document, proposal, _) =
                waiting.lock().expect("the documents waiting").remove(&pending).ok_or_else(|| {
                    ApiError(axum::http::StatusCode::NOT_FOUND, "no document waits under this name".into())
                })?;
            if of != books {
                return Err(ApiError::bad("the document was read for other books"));
            }
            let booking = match (request.book, proposal) {
                (true, Some(mut booking)) => {
                    if let Some(date) = request.date {
                        booking.date = date.parse().map_err(|_| ApiError::bad("not a date YYYY-MM-DD"))?;
                    }
                    if let Some(text) = request.text {
                        booking.text = text;
                    }
                    Some(booking)
                }
                (true, None) => return Err(ApiError::bad("there is no proposal to book")),
                (false, _) => None,
            };
            let events = workspace.record_document(&books, document, booking)?;
            Ok(dto::Written { events: events.iter().map(ToString::to_string).collect() })
        })
        .await?;
    api.changed(&id);
    Ok(Json(written))
}

/// The original of a document, byte for byte.
#[utoipa::path(get, path = "/api/v1/books/{books}/documents/{document}/original",
    params(("books" = String, Path), ("document" = String, Path)),
    responses((status = 200, content_type = "application/octet-stream"), (status = 404, body = dto::Problem)))]
async fn original(
    State(api): State<Api>,
    Path((books, document)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let (_, found) = workspace
            .documents(&books)?
            .into_iter()
            .find(|(_, d)| d.id.as_str() == document)
            .ok_or_else(|| ApiError(axum::http::StatusCode::NOT_FOUND, "no such document".into()))?;
        let bytes = workspace
            .original(&found.original.hash)?
            .ok_or_else(|| ApiError(axum::http::StatusCode::NOT_FOUND, "the original is not on this device".into()))?;
        let name = found.original.name.clone().unwrap_or_else(|| found.id.to_string()).replace('"', "");
        Ok((
            [
                (header::CONTENT_TYPE, found.original.media_type.clone()),
                (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\"")),
            ],
            bytes,
        )
            .into_response())
    })
    .await
}

fn open_view(item: &OpenEntry, names: &BTreeMap<AccountId, (Option<String>, String)>) -> dto::OpenEntry {
    dto::OpenEntry {
        document: item.reference.document.to_string(),
        entry: item.reference.entry,
        iban: item.iban.clone(),
        date: item.entry.booked.to_string(),
        text: item.text(),
        amount: dto::Amount::from(&item.entry.amount),
        suggestion: item.suggestion.as_ref().map(|s| dto::SuggestionView {
            source: match s.source {
                Source::Invoice { .. } => "invoice".into(),
                Source::History(_) => "history".into(),
                Source::Booked(_) => "booked".into(),
            },
            number: match &s.source {
                Source::Invoice { number, .. } => number.clone(),
                Source::History(_) | Source::Booked(_) => None,
            },
            booking: booking_view(&s.booking, names),
        }),
    }
}

/// The entries of bank statements not booked yet, each with a proposal where there is one.
#[utoipa::path(get, path = "/api/v1/books/{books}/bank/entries", params(("books" = String, Path)),
    responses((status = 200, body = Vec<dto::OpenEntry>)))]
async fn open_entries(
    State(api): State<Api>,
    Path(books): Path<String>,
) -> Result<Json<Vec<dto::OpenEntry>>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let names = names(workspace, &books, &owner)?;
        Ok(Json(workspace.open_entries(&books, &owner)?.iter().map(|item| open_view(item, &names)).collect()))
    })
    .await
}

/// How a statement is read.
#[derive(Debug, Deserialize, IntoParams)]
pub struct ImportQuery {
    /// The file name.
    name: Option<String>,
    /// The IBAN of the account, for a CSV file that does not name it.
    iban: Option<String>,
}

/// Reads a file of statements – camt.053, MT940 or a bank's CSV file – and records each statement.
#[utoipa::path(post, path = "/api/v1/books/{books}/bank/statements", params(("books" = String, Path), ImportQuery),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 200, body = dto::Imported), (status = 422, body = dto::Problem)))]
async fn import(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<ImportQuery>,
    bytes: Bytes,
) -> Result<Json<dto::Imported>, ApiError> {
    let id = books.clone();
    let imported = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let owner = owner(workspace, &books)?;
            let iban = query.iban.as_deref().filter(|i| !i.trim().is_empty());
            let imported = workspace.import_statements(&books, &owner, &bytes, query.name.as_deref(), iban)?;
            Ok(dto::Imported {
                documents: imported.documents.iter().map(ToString::to_string).collect(),
                known: imported.known,
                entries: imported.entries,
                shown_before: imported.shown_before,
            })
        })
        .await?;
    api.changed(&id);
    Ok(Json(imported))
}

/// The open entry named, from the books.
fn find(
    workspace: &Workspace,
    books: &BooksId,
    owner: &PartyId,
    document: &str,
    entry: u32,
) -> Result<OpenEntry, ApiError> {
    let reference = EntryReference { document: document.parse().map_err(|_| ApiError::bad("not a document"))?, entry };
    workspace
        .open_entries(books, owner)?
        .into_iter()
        .find(|item| item.reference == reference)
        .ok_or_else(|| ApiError(axum::http::StatusCode::NOT_FOUND, "no such open entry".into()))
}

/// Books an entry on an account with a tax code, its amount as the gross amount.
#[utoipa::path(post, path = "/api/v1/books/{books}/bank/entries/book", params(("books" = String, Path)),
    request_body = dto::BookEntry, responses((status = 200, body = dto::Written), (status = 422, body = dto::Problem)))]
async fn book_entry(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(request): Json<dto::BookEntry>,
) -> Result<Json<dto::Written>, ApiError> {
    let id = books.clone();
    let written = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let owner = owner(workspace, &books)?;
            let item = find(workspace, &books, &owner, &request.document, request.entry)?;
            let choices = workspace.account_choices(&books, &owner, today(), "de")?;
            let account = choices
                .iter()
                .find(|c| {
                    c.number.as_deref() == Some(request.account.trim()) || c.id.as_str() == request.account.trim()
                })
                .map(|c| c.id.clone())
                .ok_or_else(|| ApiError::bad(format!("no such account: {:?}", request.account)))?;
            let tax = request
                .tax
                .filter(|t| !t.is_empty())
                .map(|t| t.parse())
                .transpose()
                .map_err(|_| ApiError::bad("not a tax code"))?;
            let booking = workspace.entry_booking(&books, &owner, &item, account, tax, request.text)?;
            Ok(dto::Written { events: vec![workspace.execute(&books, Command::Book(booking))?.to_string()] })
        })
        .await?;
    api.changed(&id);
    Ok(Json(written))
}

/// Books an entry by its proposal.
#[utoipa::path(post, path = "/api/v1/books/{books}/bank/entries/accept", params(("books" = String, Path)),
    request_body = dto::EntryRef, responses((status = 200, body = dto::Written), (status = 422, body = dto::Problem)))]
async fn accept(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(request): Json<dto::EntryRef>,
) -> Result<Json<dto::Written>, ApiError> {
    let id = books.clone();
    let written = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let owner = owner(workspace, &books)?;
            let item = find(workspace, &books, &owner, &request.document, request.entry)?;
            let suggestion = item.suggestion.ok_or_else(|| ApiError::bad("the entry has no proposal"))?;
            let events = workspace.accept_suggestion(&books, &suggestion)?;
            Ok(dto::Written { events: events.iter().map(ToString::to_string).collect() })
        })
        .await?;
    api.changed(&id);
    Ok(Json(written))
}

/// Books every open entry that has a proposal, all or none.
#[utoipa::path(post, path = "/api/v1/books/{books}/bank/accept-all", params(("books" = String, Path)),
    responses((status = 200, body = dto::Accepted), (status = 422, body = dto::Problem)))]
async fn accept_all(State(api): State<Api>, Path(books): Path<String>) -> Result<Json<dto::Accepted>, ApiError> {
    let id = books.clone();
    let count = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let owner = owner(workspace, &books)?;
            Ok(workspace.accept_suggestions(&books, &owner)?)
        })
        .await?;
    api.changed(&id);
    Ok(Json(dto::Accepted { count }))
}

/// Fetches statements from the bank by FinTS – waits for the product registration of falkOSS, answers 501 until then.
#[utoipa::path(post, path = "/api/v1/books/{books}/bank/fetch", params(("books" = String, Path)),
    responses((status = 501, body = dto::Problem)))]
async fn fetch(State(api): State<Api>, Path(books): Path<String>) -> Result<Json<dto::Imported>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let imported = workspace.fetch_statements(&books, "")?;
        Ok(Json(dto::Imported {
            documents: imported.documents.iter().map(ToString::to_string).collect(),
            known: imported.known,
            entries: imported.entries,
            shown_before: imported.shown_before,
        }))
    })
    .await
}

/// A period.
#[derive(Debug, Deserialize, IntoParams)]
pub struct Period {
    /// The first day, `YYYY-MM-DD`.
    from: String,
    /// The last day, `YYYY-MM-DD`.
    to: String,
}

/// The data transfer of the GoBD (Z3) for a period, as a ZIP file: `index.xml` and its CSV files.
#[utoipa::path(get, path = "/api/v1/books/{books}/gobd", params(("books" = String, Path), Period),
    responses((status = 200, content_type = "application/zip"), (status = 422, body = dto::Problem)))]
async fn gobd(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(period): Query<Period>,
) -> Result<Response, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let date = |text: &str| text.parse().map_err(|_| ApiError::bad(format!("not a date YYYY-MM-DD: {text:?}")));
        let files = workspace.gobd_files(&books, &owner, date(&period.from)?, date(&period.to)?)?;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let failed = |e: &dyn std::fmt::Display| ApiError(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
        for file in files {
            zip.start_file(file.path, options).map_err(|e| failed(&e))?;
            zip.write_all(&file.content).map_err(|e| failed(&e))?;
        }
        let bytes = zip.finish().map_err(|e| failed(&e))?.into_inner();
        let name = format!("gobd-{}-{}.zip", period.from, period.to);
        Ok((
            [
                (header::CONTENT_TYPE, "application/zip".to_owned()),
                (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\"")),
            ],
            bytes,
        )
            .into_response())
    })
    .await
}
