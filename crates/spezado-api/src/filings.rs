// SPDX-License-Identifier: AGPL-3.0-or-later
//! The routes of filings: prepared from the books, submitted through the adapter of their country, asked about; and
//! the adapters themselves.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use librotenado::{BooksId, Country, FilingId, FilingKind, FilingState, Names, PartyId, SubmissionState};
use serde::Deserialize;
use spezado_app::{FilingItem, Workspace};
use utoipa::IntoParams;

use crate::{Api, ApiError, dto};

/// The routes of filings and adapters.
pub fn routes() -> Router<Api> {
    Router::new()
        .route("/books/{books}/filings", get(filings).post(prepare))
        .route("/books/{books}/filings/{filing}/submit", post(submit))
        .route("/books/{books}/filings/{filing}/refresh", post(refresh))
        .route("/adapters/{country}", get(adapter).put(set_adapter))
}

/// The language asked for.
#[derive(Debug, Deserialize, IntoParams)]
pub struct LangQuery {
    /// `de` or `en`; German when none.
    lang: Option<String>,
}

fn books_id(text: &str) -> Result<BooksId, ApiError> {
    text.parse().map_err(|_| ApiError::bad(format!("not the identity of books: {text:?}")))
}

fn filing_id(text: &str) -> Result<FilingId, ApiError> {
    text.parse().map_err(|_| ApiError::bad(format!("not the identity of a filing: {text:?}")))
}

fn country(text: &str) -> Result<Country, ApiError> {
    text.parse().map_err(|_| ApiError::bad(format!("not a country, ISO 3166-1 alpha-2: {text:?}")))
}

fn owner(workspace: &Workspace, books: &BooksId) -> Result<PartyId, ApiError> {
    let owners = workspace.get(books)?.events().owners();
    owners.first().map(|o| (*o).clone()).ok_or_else(|| ApiError::bad("the books have no owner"))
}

fn kind(kind: FilingKind) -> &'static str {
    match kind {
        FilingKind::VatReturn => "vat_return",
        FilingKind::CashBasisStatement => "cash_basis_statement",
        FilingKind::AnnualAccounts => "annual_accounts",
        FilingKind::Unknown => "unknown",
    }
}

fn state(state: FilingState) -> &'static str {
    match state {
        FilingState::Prepared => "prepared",
        FilingState::Submitted => "submitted",
        FilingState::Accepted => "accepted",
        FilingState::Rejected => "rejected",
    }
}

fn view(item: FilingItem, names: Option<&Names>) -> dto::FilingView {
    let filing = item.filing;
    let content = filing.content.first();
    let fields = content
        .map(|content| {
            content
                .fields
                .iter()
                .map(|(field, amount)| dto::Field {
                    field: field.clone(),
                    name: names.and_then(|n| n.get(&format!("{}/{field}", content.report))).map(str::to_owned),
                    amount: dto::Amount::from(amount),
                })
                .collect()
        })
        .unwrap_or_default();
    dto::FilingView {
        id: filing.id.to_string(),
        kind: kind(filing.kind).into(),
        report: content.map(|c| c.report.clone()).unwrap_or_default(),
        from: filing.from.to_string(),
        to: filing.to.to_string(),
        state: state(item.state).into(),
        outdated: item.outdated,
        corrects: filing.corrects.map(|id| id.to_string()),
        documents: item.documents.iter().map(ToString::to_string).collect(),
        fields,
    }
}

fn submission_view(status: librotenado::SubmissionStatus, test: bool) -> dto::SubmissionView {
    dto::SubmissionView {
        state: match status.state {
            SubmissionState::Submitted => "submitted",
            SubmissionState::Accepted => "accepted",
            SubmissionState::Rejected => "rejected",
        }
        .into(),
        test,
        message: status.message,
        receipts: status.receipts.into_iter().map(|r| r.name).collect(),
    }
}

/// The filings of the books, in the order they were prepared.
#[utoipa::path(get, path = "/api/v1/books/{books}/filings", params(("books" = String, Path), LangQuery),
    responses((status = 200, body = Vec<dto::FilingView>)))]
pub(crate) async fn filings(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<LangQuery>,
) -> Result<Json<Vec<dto::FilingView>>, ApiError> {
    api.run(move |workspace| {
        let names = Names::v0_1(query.lang.as_deref().unwrap_or("de"));
        let items = workspace.filings(&books_id(&books)?)?;
        Ok(Json(items.into_iter().map(|item| view(item, names.as_ref())).collect()))
    })
    .await
}

/// Prepares the filing of a report for a period from the books as they are – the same filing again if one with
/// that content waits to be submitted, a correction if one was submitted.
#[utoipa::path(post, path = "/api/v1/books/{books}/filings", params(("books" = String, Path), LangQuery),
    request_body = dto::PrepareFiling,
    responses((status = 200, body = dto::FilingView), (status = 404, body = dto::Problem), (status = 422, body = dto::Problem)))]
pub(crate) async fn prepare(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<LangQuery>,
    Json(request): Json<dto::PrepareFiling>,
) -> Result<Json<dto::FilingView>, ApiError> {
    let changed = books.clone();
    let view = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let owner = owner(workspace, &books)?;
            let from = request.from.trim().parse().map_err(|_| ApiError::bad("not a date YYYY-MM-DD"))?;
            let to = request.to.trim().parse().map_err(|_| ApiError::bad("not a date YYYY-MM-DD"))?;
            let filing = workspace.prepare_filing(&books, &owner, &request.report, from, to)?;
            let item = workspace.filings(&books)?.into_iter().find(|item| item.filing.id == filing.id);
            let item = item.ok_or_else(|| ApiError::not_found("the filing just prepared"))?;
            Ok(view(item, Names::v0_1(query.lang.as_deref().unwrap_or("de")).as_ref()))
        })
        .await?;
    api.changed(&changed);
    Ok(Json(view))
}

/// Submits a prepared filing through the adapter of its country. A test submission is not processed by the
/// authority and changes nothing in the books.
#[utoipa::path(post, path = "/api/v1/books/{books}/filings/{filing}/submit",
    params(("books" = String, Path), ("filing" = String, Path)), request_body = dto::Submit,
    responses((status = 200, body = dto::SubmissionView), (status = 422, body = dto::Problem)))]
pub(crate) async fn submit(
    State(api): State<Api>,
    Path((books, filing)): Path<(String, String)>,
    Json(request): Json<dto::Submit>,
) -> Result<Json<dto::SubmissionView>, ApiError> {
    let changed = books.clone();
    let status = api
        .run(move |workspace| Ok(workspace.submit_filing(&books_id(&books)?, &filing_id(&filing)?, request.test)?))
        .await?;
    if !request.test {
        api.changed(&changed);
    }
    Ok(Json(submission_view(status, request.test)))
}

/// Asks the adapter where a submitted filing stands and records an answer.
#[utoipa::path(post, path = "/api/v1/books/{books}/filings/{filing}/refresh",
    params(("books" = String, Path), ("filing" = String, Path)),
    responses((status = 200, body = dto::SubmissionView), (status = 422, body = dto::Problem)))]
pub(crate) async fn refresh(
    State(api): State<Api>,
    Path((books, filing)): Path<(String, String)>,
) -> Result<Json<dto::SubmissionView>, ApiError> {
    let changed = books.clone();
    let status =
        api.run(move |workspace| Ok(workspace.refresh_filing(&books_id(&books)?, &filing_id(&filing)?)?)).await?;
    api.changed(&changed);
    Ok(Json(submission_view(status, false)))
}

/// The adapter that submits filings in a country, if one is set; never its token.
#[utoipa::path(get, path = "/api/v1/adapters/{country}", params(("country" = String, Path)),
    responses((status = 200, body = dto::AdapterView)))]
pub(crate) async fn adapter(
    State(api): State<Api>,
    Path(code): Path<String>,
) -> Result<Json<dto::AdapterView>, ApiError> {
    api.run(move |workspace| {
        let country = country(&code)?;
        Ok(Json(dto::AdapterView { country: country.to_string(), url: workspace.adapter(country)? }))
    })
    .await
}

/// Sets the adapter that submits filings in a country: its address and the token spezado authenticates itself with.
#[utoipa::path(put, path = "/api/v1/adapters/{country}", params(("country" = String, Path)),
    request_body = dto::SetAdapter, responses((status = 200, body = dto::AdapterView), (status = 422, body = dto::Problem)))]
pub(crate) async fn set_adapter(
    State(api): State<Api>,
    Path(code): Path<String>,
    Json(request): Json<dto::SetAdapter>,
) -> Result<Json<dto::AdapterView>, ApiError> {
    api.run(move |workspace| {
        let country = country(&code)?;
        workspace.set_adapter(country, &request.url, &request.token)?;
        Ok(Json(dto::AdapterView { country: country.to_string(), url: workspace.adapter(country)? }))
    })
    .await
}
