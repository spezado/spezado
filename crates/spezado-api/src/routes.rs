// SPDX-License-Identifier: AGPL-3.0-or-later
//! The routes of the API and their description in OpenAPI.

use std::{collections::BTreeMap, convert::Infallible};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::sse::{Event, KeepAlive, Sse},
    routing::{delete, get, post},
};
use futures_util::Stream;
use librotenado::{AccountId, Booking, BooksId, Date, Money, Names, PartyId, Person, Posting};
use serde::Deserialize;
use spezado_app::{Party, SimpleEntry, Workspace, today};
use spezado_core::{Command, parse_amount};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};
use utoipa::{IntoParams, OpenApi};

use crate::{Api, ApiError, dto};

/// The description of the API in OpenAPI.
#[derive(Debug, OpenApi)]
#[openapi(
    info(title = "spezado", description = "The open API of spezado: free accounting for everyone."),
    paths(
        pair,
        regions,
        devices,
        revoke,
        events,
        books,
        set_up,
        overview,
        journal,
        accounts,
        balances,
        tax_codes,
        book_simple,
        book_lines,
        opening,
        book_opening,
        deadlines,
        change_deadlines,
        reverse,
        reports,
        report,
        review,
        close,
        crate::records::documents,
        crate::records::read_document,
        crate::records::record,
        crate::records::original,
        crate::records::open_entries,
        crate::records::import,
        crate::records::book_entry,
        crate::records::accept,
        crate::records::accept_all,
        crate::records::fetch,
        crate::records::gobd,
        crate::invoices::profile,
        crate::invoices::set_profile,
        crate::invoices::customers,
        crate::invoices::save_customer,
        crate::invoices::next_number,
        crate::invoices::issue,
        crate::sync::sync_status,
        crate::sync::sync_now,
        crate::sync::sync_start,
        crate::sync::sync_invite,
        crate::sync::sync_devices,
        crate::sync::sync_join,
        crate::sync::sync_recover
    ),
    components(schemas(dto::Problem))
)]
pub struct ApiDoc;

/// The routes anyone on the device may call.
pub fn open() -> Router<Api> {
    Router::new().route("/openapi.json", get(openapi)).route("/pair", post(pair))
}

/// The routes for paired devices.
pub fn protected() -> Router<Api> {
    Router::new()
        .route("/regions", get(regions))
        .route("/devices", get(devices))
        .route("/devices/{device}", delete(revoke))
        .route("/events", get(events))
        .route("/books", get(books).post(set_up))
        .route("/books/{books}/overview", get(overview))
        .route("/books/{books}/journal", get(journal))
        .route("/books/{books}/accounts", get(accounts))
        .route("/books/{books}/balances", get(balances))
        .route("/books/{books}/tax-codes", get(tax_codes))
        .route("/books/{books}/bookings/simple", post(book_simple))
        .route("/books/{books}/bookings/lines", post(book_lines))
        .route("/books/{books}/opening", get(opening).post(book_opening))
        .route("/books/{books}/deadlines", get(deadlines).put(change_deadlines))
        .route("/books/{books}/reversals", post(reverse))
        .route("/books/{books}/reports", get(reports))
        .route("/books/{books}/reports/{report}", get(report))
        .route("/books/{books}/review", get(review))
        .route("/books/{books}/closings", post(close))
}

/// The states whose public holidays can move deadlines.
#[utoipa::path(get, path = "/api/v1/regions", params(LangQuery), responses((status = 200, body = Vec<dto::RegionView>)))]
async fn regions(Query(query): Query<LangQuery>) -> Json<Vec<dto::RegionView>> {
    let english = query.lang.as_deref() == Some("en");
    Json(
        spezado_app::GERMAN_STATES
            .iter()
            .map(|(code, de, en)| dto::RegionView {
                code: (*code).into(),
                name: (if english { *en } else { *de }).into(),
            })
            .collect(),
    )
}

async fn openapi() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

fn date(text: &str) -> Result<Date, ApiError> {
    text.trim().parse().map_err(|_| ApiError::bad(format!("not a date YYYY-MM-DD: {text:?}")))
}

fn eur() -> librotenado::Currency {
    "EUR".parse().expect("a currency")
}

fn books_id(text: &str) -> Result<BooksId, ApiError> {
    text.parse().map_err(|_| ApiError::bad(format!("not the identity of books: {text:?}")))
}

/// The owner shown of a set of books: the first.
fn owner(workspace: &Workspace, books: &BooksId) -> Result<PartyId, ApiError> {
    let owners = workspace.get(books)?.events().owners();
    owners.first().map(|o| (*o).clone()).ok_or_else(|| ApiError::bad("the books have no owner"))
}

/// An account by its national number or identity.
fn account(workspace: &Workspace, books: &BooksId, owner: &PartyId, text: &str) -> Result<AccountId, ApiError> {
    let choices = workspace.account_choices(books, owner, today(), "de")?;
    choices
        .iter()
        .find(|c| c.number.as_deref() == Some(text.trim()) || c.id.as_str() == text.trim())
        .map(|c| c.id.clone())
        .ok_or_else(|| ApiError::bad(format!("no such account: {text:?}")))
}

fn year_range(year: Option<u16>) -> (u16, Date, Date) {
    let year = year.unwrap_or_else(|| today().year());
    (year, Date::new(year, 1, 1).expect("a date"), Date::new(year, 12, 31).expect("a date"))
}

/// The year asked for; this year when none is.
#[derive(Debug, Deserialize, IntoParams)]
pub struct YearQuery {
    /// The calendar year.
    year: Option<u16>,
    /// The language of names: `de` or `en`.
    lang: Option<String>,
}

/// The language asked for.
#[derive(Debug, Deserialize, IntoParams)]
pub struct LangQuery {
    /// `de` or `en`; German when none.
    lang: Option<String>,
}

/// A period.
#[derive(Debug, Deserialize, IntoParams)]
pub struct PeriodQuery {
    /// The first day, `YYYY-MM-DD`.
    from: String,
    /// The last day, `YYYY-MM-DD`.
    to: String,
    /// `de` or `en`.
    lang: Option<String>,
}

/// Pairs a device with the code `spezado serve` shows; returns its token, once.
#[utoipa::path(post, path = "/api/v1/pair", request_body = dto::PairRequest,
    responses((status = 200, body = dto::Paired), (status = 403, body = dto::Problem)))]
async fn pair(State(api): State<Api>, Json(request): Json<dto::PairRequest>) -> Result<Json<dto::Paired>, ApiError> {
    api.pair(request).await.map(Json)
}

/// The devices paired.
#[utoipa::path(get, path = "/api/v1/devices", responses((status = 200, body = Vec<dto::Device>)))]
async fn devices(State(api): State<Api>) -> Result<Json<Vec<dto::Device>>, ApiError> {
    let devices = api.run(|workspace| Api::devices(workspace)).await?;
    Ok(Json(devices.into_iter().map(|d| dto::Device { id: d.id, name: d.name, paired: d.paired }).collect()))
}

/// Revokes a device's token.
#[utoipa::path(delete, path = "/api/v1/devices/{device}", params(("device" = String, Path)),
    responses((status = 200), (status = 404, body = dto::Problem)))]
async fn revoke(State(api): State<Api>, Path(device): Path<String>) -> Result<(), ApiError> {
    api.run(move |workspace| {
        let mut devices = Api::devices(workspace)?;
        let before = devices.len();
        devices.retain(|d| d.id != device);
        if devices.len() == before {
            return Err(ApiError::not_found("no such device"));
        }
        Api::store_devices(workspace, &devices)
    })
    .await
}

/// Server-Sent Events: `changed` with the books whenever something was written – read again what is shown. A browser
/// passes its token as the query `token`.
#[utoipa::path(get, path = "/api/v1/events", responses((status = 200, description = "text/event-stream of dto::Change")))]
async fn events(State(api): State<Api>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(api.changes.subscribe()).filter_map(|change| {
        let change = change.ok()?;
        Some(Ok(Event::default().event("changed").json_data(change).expect("a change in JSON")))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// The books of the workspace.
#[utoipa::path(get, path = "/api/v1/books", responses((status = 200, body = Vec<dto::BooksSummary>)))]
async fn books(State(api): State<Api>) -> Result<Json<Vec<dto::BooksSummary>>, ApiError> {
    api.run(|workspace| {
        let mut list = Vec::new();
        for (id, books) in workspace.books() {
            let owner = books.events().owners().first().map(|o| (*o).clone());
            let (organisations, persons) = books.events().parties();
            let name = owner.as_ref().and_then(|o| {
                persons.get(o).map(|p| p.name.clone()).or_else(|| organisations.get(o).map(|g| g.name.clone()))
            });
            let chart = match &owner {
                Some(o) => workspace.national_chart(id, o, today()).ok().flatten().map(|t| t.chart),
                None => None,
            };
            list.push(dto::BooksSummary {
                id: id.to_string(),
                owner: owner.map(|o| o.to_string()),
                owner_name: name,
                chart,
            });
        }
        Ok(Json(list))
    })
    .await
}

/// Sets up books for a person.
#[utoipa::path(post, path = "/api/v1/books", request_body = dto::SetUp,
    responses((status = 200, body = dto::BooksSummary), (status = 422, body = dto::Problem)))]
async fn set_up(State(api): State<Api>, Json(setup): Json<dto::SetUp>) -> Result<Json<dto::BooksSummary>, ApiError> {
    let summary = api
        .run(move |workspace| {
            let id: PartyId = spezado_app::new_id();
            let period = if setup.vat_scheme == "de-small-business" { "year" } else { setup.vat_period.as_str() };
            let mut configuration = serde_json::json!({
                "owner": id.as_str(), "from": setup.from, "currency": "EUR", "year_start": 1,
                "accounting": setup.accounting, "chart": setup.chart,
                "vat": {"scheme": setup.vat_scheme, "basis": setup.vat_basis, "period": period},
            });
            if setup.vat_extended && setup.vat_scheme != "de-small-business" {
                configuration["vat"]["extended"] = true.into();
            }
            if let Some(region) = setup.region.as_ref().filter(|r| !r.is_empty()) {
                configuration["region"] = region.as_str().into();
            }
            let configuration = serde_json::from_value(configuration).map_err(|e| ApiError::bad(e.to_string()))?;
            let person =
                Person { id: id.clone(), name: setup.name.clone(), tax_ids: vec![], roles: vec![], contact: None };
            let books = workspace.set_up(Party::Person(person), configuration)?;
            Ok(dto::BooksSummary {
                id: books.to_string(),
                owner: Some(id.to_string()),
                owner_name: Some(setup.name),
                chart: Some(setup.chart),
            })
        })
        .await?;
    api.changed(&summary.id);
    Ok(Json(summary))
}

/// The books at a glance, for a year.
#[utoipa::path(get, path = "/api/v1/books/{books}/overview", params(("books" = String, Path), YearQuery),
    responses((status = 200, body = dto::Overview), (status = 404, body = dto::Problem)))]
async fn overview(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<YearQuery>,
) -> Result<Json<dto::Overview>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let (year, first, last) = year_range(query.year);
        let set = workspace.get(&books)?;
        let events = set.events();
        let money = |units: i128| dto::Amount::from(&Money::new(units, eur()).expect("an amount"));
        let mut overview = dto::Overview {
            year,
            bank: None,
            cash: None,
            income: None,
            expenses: None,
            next_vat_return: None,
            open_entries: workspace.open_entries(&books, &owner)?.len(),
            issues: workspace.review(&books)?.len(),
            closed_until: events.closed_until(&owner).map(|d| d.to_string()),
        };
        if let Ok(standard) = workspace.standard_accounts(&books, &owner, today()) {
            let balances = events.functional_balances(Some(last)).map_err(|e| ApiError::bad(e.to_string()))?;
            overview.bank = Some(money(workspace.bank_holdings(&books, &owner, last)?));
            overview.cash = Some(money(balances.get(&owner, &standard.cash, &eur())));
            let (income, expenses) = set.income_and_expenses(workspace.chart(), &owner, first, last);
            overview.income = Some(money(income));
            overview.expenses = Some(money(expenses));
        }
        let returns = workspace.vat_returns(&books, &owner, today().year()).unwrap_or_default();
        overview.next_vat_return = returns.into_iter().find(|r| r.due >= today()).map(|r| dto::VatReturnDue {
            from: r.from.to_string(),
            to: r.to.to_string(),
            due: r.due.to_string(),
        });
        Ok(Json(overview))
    })
    .await
}

/// The names of accounts by identity, with their national numbers.
fn account_names(
    workspace: &Workspace,
    books: &BooksId,
    owner: &PartyId,
    lang: &str,
) -> Result<BTreeMap<AccountId, (Option<String>, String)>, ApiError> {
    Ok(workspace
        .account_choices(books, owner, today(), lang)?
        .into_iter()
        .map(|c| (c.id, (c.number, c.name)))
        .collect())
}

/// The bookings of a year, by date.
#[utoipa::path(get, path = "/api/v1/books/{books}/journal", params(("books" = String, Path), YearQuery),
    responses((status = 200, body = Vec<dto::JournalEntry>)))]
async fn journal(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<YearQuery>,
) -> Result<Json<Vec<dto::JournalEntry>>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let (_, first, last) = year_range(query.year);
        let names = account_names(workspace, &books, &owner, query.lang.as_deref().unwrap_or("de"))?;
        let entries = workspace.get(&books)?.journal(Some(&owner), Some(first), Some(last));
        Ok(Json(
            entries
                .into_iter()
                .map(|entry| {
                    let debit: i128 = entry.booking.postings.iter().map(|p| p.amount.base_units().max(0)).sum();
                    let currency = entry.booking.postings[0].amount.currency().clone();
                    dto::JournalEntry {
                        event: entry.event.to_string(),
                        date: entry.booking.date.to_string(),
                        text: entry.booking.text.clone(),
                        amount: dto::Amount::from(&Money::new(debit, currency).expect("an amount")),
                        reverses: entry.reverses.map(|e| e.to_string()),
                        reversed_by: entry.reversed_by.map(|e| e.to_string()),
                        postings: entry
                            .booking
                            .postings
                            .iter()
                            .map(|p| {
                                let (number, name) =
                                    names.get(&p.account).cloned().unwrap_or((None, p.account.to_string()));
                                dto::PostingView {
                                    account: p.account.to_string(),
                                    number,
                                    name,
                                    amount: dto::Amount::from(&p.amount),
                                    tax: p.tax.as_ref().map(ToString::to_string),
                                }
                            })
                            .collect(),
                        documents: entry.booking.documents.iter().map(ToString::to_string).collect(),
                    }
                })
                .collect(),
        ))
    })
    .await
}

/// The accounts to book on, by national number.
#[utoipa::path(get, path = "/api/v1/books/{books}/accounts", params(("books" = String, Path), LangQuery),
    responses((status = 200, body = Vec<dto::AccountView>)))]
async fn accounts(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<LangQuery>,
) -> Result<Json<Vec<dto::AccountView>>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let choices = workspace.account_choices(&books, &owner, today(), query.lang.as_deref().unwrap_or("de"))?;
        Ok(Json(
            choices
                .into_iter()
                .map(|c| dto::AccountView {
                    id: c.id.to_string(),
                    number: c.number,
                    name: c.name,
                    kind: format!("{:?}", c.kind).to_lowercase(),
                })
                .collect(),
        ))
    })
    .await
}

/// The balances of the accounts at the end of a year, those that have one.
#[utoipa::path(get, path = "/api/v1/books/{books}/balances", params(("books" = String, Path), YearQuery),
    responses((status = 200, body = Vec<dto::BalanceView>)))]
async fn balances(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<YearQuery>,
) -> Result<Json<Vec<dto::BalanceView>>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let (_, _, last) = year_range(query.year);
        let names = account_names(workspace, &books, &owner, query.lang.as_deref().unwrap_or("de"))?;
        let balances = workspace
            .get(&books)?
            .events()
            .functional_balances(Some(last))
            .map_err(|e| ApiError::bad(e.to_string()))?;
        let mut list: Vec<dto::BalanceView> = balances
            .iter()
            .filter(|(party, _, _, units)| **party == owner && *units != 0)
            .map(|(_, account, currency, units)| {
                let (number, name) = names.get(account).cloned().unwrap_or((None, account.to_string()));
                dto::BalanceView {
                    account: account.to_string(),
                    number,
                    name,
                    balance: dto::Amount::from(&Money::new(units, currency.clone()).expect("a balance")),
                }
            })
            .collect();
        list.sort_by(|a, b| {
            (a.number.as_ref().map(String::len), &a.number).cmp(&(b.number.as_ref().map(String::len), &b.number))
        });
        Ok(Json(list))
    })
    .await
}

/// The tax codes to choose from.
#[utoipa::path(get, path = "/api/v1/books/{books}/tax-codes", params(("books" = String, Path), LangQuery),
    responses((status = 200, body = Vec<dto::TaxCodeView>)))]
async fn tax_codes(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<LangQuery>,
) -> Result<Json<Vec<dto::TaxCodeView>>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let names = Names::v0_1(query.lang.as_deref().unwrap_or("de"));
        let small = workspace
            .get(&books)?
            .events()
            .configuration(&owner, today())
            .and_then(|c| c.vat.as_ref())
            .is_some_and(|vat| vat.scheme.as_str() == "de-small-business");
        let codes = [
            "de-in-19",
            "de-in-7",
            "de-out-19",
            "de-out-7",
            "de-out-0",
            "de-out-exempt",
            "de-out-13b",
            "de-out-eu-service",
            "de-out-ig-supply",
            "de-out-export",
            "de-out-small-business",
            "de-in-no-deduction",
        ];
        Ok(Json(
            codes
                .iter()
                .filter(|code| !small || !code.starts_with("de-out-") || **code == "de-out-small-business")
                .map(|code| dto::TaxCodeView {
                    code: (*code).to_owned(),
                    name: names.as_ref().and_then(|n| n.get(code)).unwrap_or(code).to_owned(),
                })
                .collect(),
        ))
    })
    .await
}

/// Books a gross amount on an account, split by its tax code, against the bank or the cash.
#[utoipa::path(post, path = "/api/v1/books/{books}/bookings/simple", params(("books" = String, Path)),
    request_body = dto::SimpleBooking, responses((status = 200, body = dto::Written), (status = 422, body = dto::Problem)))]
async fn book_simple(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(request): Json<dto::SimpleBooking>,
) -> Result<Json<dto::Written>, ApiError> {
    let id = books.clone();
    let written = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let owner = owner(workspace, &books)?;
            let date = date(&request.date)?;
            let standard = workspace.standard_accounts(&books, &owner, date)?;
            let payment = match request.payment.as_str() {
                "bank" => standard.bank,
                "cash" => standard.cash,
                other => return Err(ApiError::bad(format!("payment is bank or cash, not {other:?}"))),
            };
            let entry = SimpleEntry {
                owner: owner.clone(),
                date,
                text: request.text,
                account: account(workspace, &books, &owner, &request.account)?,
                gross: parse_amount(&request.gross, &eur()).map_err(|e| ApiError::bad(e.to_string()))?,
                tax: request
                    .tax
                    .filter(|t| !t.is_empty())
                    .map(|t| t.parse())
                    .transpose()
                    .map_err(|_| ApiError::bad("not a tax code"))?,
                payment,
                documents: request
                    .documents
                    .iter()
                    .map(|d| d.parse())
                    .collect::<Result<_, _>>()
                    .map_err(|_| ApiError::bad("not the identity of a document"))?,
            };
            let booking = workspace.simple_booking(&books, &entry)?;
            Ok(dto::Written { events: vec![workspace.execute(&books, Command::Book(booking))?.to_string()] })
        })
        .await?;
    api.changed(&id);
    Ok(Json(written))
}

/// Books lines that balance.
#[utoipa::path(post, path = "/api/v1/books/{books}/bookings/lines", params(("books" = String, Path)),
    request_body = dto::LinesBooking, responses((status = 200, body = dto::Written), (status = 422, body = dto::Problem)))]
async fn book_lines(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(request): Json<dto::LinesBooking>,
) -> Result<Json<dto::Written>, ApiError> {
    let id = books.clone();
    let written = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let owner = owner(workspace, &books)?;
            let postings = request
                .lines
                .iter()
                .map(|line| {
                    Ok(Posting {
                        owner: owner.clone(),
                        account: account(workspace, &books, &owner, &line.account)?,
                        amount: parse_amount(&line.amount, &eur()).map_err(|e| ApiError::bad(e.to_string()))?,
                        note: None,
                        tax: line
                            .tax
                            .as_ref()
                            .filter(|t| !t.is_empty())
                            .map(|t| t.parse())
                            .transpose()
                            .map_err(|_| ApiError::bad("not a tax code"))?,
                        exchange: None,
                    })
                })
                .collect::<Result<Vec<_>, ApiError>>()?;
            let booking = Booking {
                date: date(&request.date)?,
                text: request.text,
                postings,
                documents: vec![],
                entries: vec![],
                transaction: None,
            };
            Ok(dto::Written { events: vec![workspace.execute(&books, Command::Book(booking))?.to_string()] })
        })
        .await?;
    api.changed(&id);
    Ok(Json(written))
}

/// Reverses a booking.
#[utoipa::path(post, path = "/api/v1/books/{books}/reversals", params(("books" = String, Path)),
    request_body = dto::Reversal, responses((status = 200, body = dto::Written), (status = 422, body = dto::Problem)))]
async fn reverse(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(request): Json<dto::Reversal>,
) -> Result<Json<dto::Written>, ApiError> {
    let id = books.clone();
    let written = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let event = request.event.parse().map_err(|_| ApiError::bad("not an event"))?;
            let command = Command::Reverse { event, date: date(&request.date)?, text: request.text };
            Ok(dto::Written { events: vec![workspace.execute(&books, command)?.to_string()] })
        })
        .await?;
    api.changed(&id);
    Ok(Json(written))
}

/// The reports of a year: the advance VAT returns with their deadlines, the annual ones.
#[utoipa::path(get, path = "/api/v1/books/{books}/reports", params(("books" = String, Path), YearQuery),
    responses((status = 200, body = Vec<dto::ReportItem>)))]
async fn reports(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<YearQuery>,
) -> Result<Json<Vec<dto::ReportItem>>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let (year, first, last) = year_range(query.year);
        let mut items: Vec<dto::ReportItem> = workspace
            .vat_returns(&books, &owner, year)?
            .into_iter()
            .map(|r| dto::ReportItem {
                id: "de-ustva".into(),
                name: "Umsatzsteuer-Voranmeldung".into(),
                from: r.from.to_string(),
                to: r.to.to_string(),
                due: Some(r.due.to_string()),
            })
            .collect();
        for (id, name) in workspace.reports(&books, &owner, first, last)? {
            if id != "de-ustva" {
                items.push(dto::ReportItem {
                    id: id.into(),
                    name: name.into(),
                    from: first.to_string(),
                    to: last.to_string(),
                    due: None,
                });
            }
        }
        Ok(Json(items))
    })
    .await
}

/// The figures of a report for a period – a draft until tax advisers have reviewed its mapping.
#[utoipa::path(get, path = "/api/v1/books/{books}/reports/{report}",
    params(("books" = String, Path), ("report" = String, Path), PeriodQuery),
    responses((status = 200, body = dto::Figures), (status = 404, body = dto::Problem)))]
async fn report(
    State(api): State<Api>,
    Path((books, report)): Path<(String, String)>,
    Query(query): Query<PeriodQuery>,
) -> Result<Json<dto::Figures>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let figures = workspace.report(&books, &owner, &report, date(&query.from)?, date(&query.to)?)?;
        let names = Names::v0_1(query.lang.as_deref().unwrap_or("de"));
        let fields = figures
            .fields
            .iter()
            .map(|(field, units)| dto::Field {
                field: field.clone(),
                name: names.as_ref().and_then(|n| n.get(&format!("{}/{field}", figures.id))).map(str::to_owned),
                amount: dto::Amount::from(&Money::new(*units, figures.currency.clone()).expect("an amount")),
            })
            .collect();
        Ok(Json(dto::Figures { id: figures.id.into(), name: figures.name.into(), law: figures.law, fields }))
    })
    .await
}

/// The first day of the books and the opening balances booked.
#[utoipa::path(get, path = "/api/v1/books/{books}/opening", params(("books" = String, Path), LangQuery),
    responses((status = 200, body = dto::OpeningView)))]
async fn opening(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<LangQuery>,
) -> Result<Json<dto::OpeningView>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let names = account_names(workspace, &books, &owner, query.lang.as_deref().unwrap_or("de"))?;
        let balances = workspace
            .opening_balances(&books, &owner)?
            .into_iter()
            .map(|(account, amount)| {
                let (number, name) = names.get(&account).cloned().unwrap_or((None, account.to_string()));
                dto::OpeningBalance { account: account.to_string(), number, name, amount: dto::Amount::from(&amount) }
            })
            .collect();
        let start = workspace.books_start(&books, &owner)?.map(|d| d.to_string());
        Ok(Json(dto::OpeningView { start, balances }))
    })
    .await
}

/// Books opening balances, once, against the opening balance sheet – they count in balances, in no result.
#[utoipa::path(post, path = "/api/v1/books/{books}/opening", params(("books" = String, Path)),
    request_body = dto::Opening, responses((status = 200, body = dto::Written), (status = 422, body = dto::Problem)))]
async fn book_opening(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(request): Json<dto::Opening>,
) -> Result<Json<dto::Written>, ApiError> {
    let id = books.clone();
    let written = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let owner = owner(workspace, &books)?;
            let lines = request
                .lines
                .iter()
                .map(|line| {
                    let amount = parse_amount(&line.amount, &eur()).map_err(|e| ApiError::bad(e.to_string()))?;
                    Ok((account(workspace, &books, &owner, &line.account)?, amount))
                })
                .collect::<Result<Vec<_>, ApiError>>()?;
            let date = match &request.date {
                Some(text) => date(text)?,
                None => workspace.books_start(&books, &owner)?.unwrap_or_else(today),
            };
            let text = request.text.unwrap_or_else(|| "Anfangsbestände".into());
            Ok(dto::Written { events: vec![workspace.book_opening(&books, &owner, date, &lines, &text)?.to_string()] })
        })
        .await?;
    api.changed(&id);
    Ok(Json(written))
}

/// The facts that move the owner's deadlines today: the state whose public holidays apply and whether VAT returns
/// are extended for good.
#[utoipa::path(get, path = "/api/v1/books/{books}/deadlines", params(("books" = String, Path)),
    responses((status = 200, body = dto::Deadlines)))]
async fn deadlines(State(api): State<Api>, Path(books): Path<String>) -> Result<Json<dto::Deadlines>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let events = workspace.get(&books)?.events();
        let configuration = events.configuration(&owner, today());
        Ok(Json(dto::Deadlines {
            from: None,
            region: configuration.and_then(|c| c.region.as_ref()).map(ToString::to_string),
            vat_extended: configuration.and_then(|c| c.vat.as_ref()).is_some_and(|v| v.extended),
        }))
    })
    .await
}

/// Changes the facts that move the owner's deadlines from a date on; everything else stays as configured.
#[utoipa::path(put, path = "/api/v1/books/{books}/deadlines", params(("books" = String, Path)),
    request_body = dto::Deadlines, responses((status = 200, body = dto::Deadlines), (status = 422, body = dto::Problem)))]
async fn change_deadlines(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(request): Json<dto::Deadlines>,
) -> Result<Json<dto::Deadlines>, ApiError> {
    let id = books.clone();
    let changed = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let owner = owner(workspace, &books)?;
            let from = match &request.from {
                Some(text) => date(text)?,
                None => today(),
            };
            let region = request
                .region
                .as_deref()
                .filter(|r| !r.is_empty())
                .map(str::parse)
                .transpose()
                .map_err(|_| ApiError::bad("not a region"))?;
            workspace.change_deadlines(&books, &owner, from, region, request.vat_extended)?;
            Ok(dto::Deadlines { from: Some(from.to_string()), ..request })
        })
        .await?;
    api.changed(&id);
    Ok(Json(changed))
}

/// What needs review, each issue in a sentence of the language asked for.
#[utoipa::path(get, path = "/api/v1/books/{books}/review", params(("books" = String, Path), LangQuery),
    responses((status = 200, body = Vec<String>)))]
async fn review(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<LangQuery>,
) -> Result<Json<Vec<String>>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books).ok();
        Ok(Json(workspace.review_in(&books, owner.as_ref(), query.lang.as_deref().unwrap_or("de"))?))
    })
    .await
}

/// Closes the books of their owner up to a date.
#[utoipa::path(post, path = "/api/v1/books/{books}/closings", params(("books" = String, Path)),
    request_body = dto::Close, responses((status = 200, body = dto::Written), (status = 422, body = dto::Problem)))]
async fn close(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(request): Json<dto::Close>,
) -> Result<Json<dto::Written>, ApiError> {
    let id = books.clone();
    let written = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let owner = owner(workspace, &books)?;
            let software = workspace.software();
            let command = Command::Close { owner, to: date(&request.to)?, software };
            Ok(dto::Written { events: vec![workspace.execute(&books, command)?.to_string()] })
        })
        .await?;
    api.changed(&id);
    Ok(Json(written))
}
