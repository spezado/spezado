// SPDX-License-Identifier: AGPL-3.0-or-later
//! The routes of invoicing: the seller's details, the customers, and invoices written as XRechnung.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use librotenado::BooksId;
use serde::Deserialize;
use spezado_app::{InvoiceFormat, NewInvoice, NewLine, Workspace, today};
use spezado_core::parse_amount;
use utoipa::IntoParams;

use crate::{Api, ApiError, dto};

/// The routes of invoicing.
pub fn routes() -> Router<Api> {
    Router::new()
        .route("/books/{books}/invoicing/profile", get(profile).put(set_profile))
        .route("/books/{books}/customers", get(customers).post(save_customer))
        .route("/books/{books}/invoices", post(issue))
        .route("/books/{books}/invoices/next-number", get(next_number))
}

fn books_id(text: &str) -> Result<BooksId, ApiError> {
    text.parse().map_err(|_| ApiError::bad(format!("not the identity of books: {text:?}")))
}

fn owner(workspace: &Workspace, books: &BooksId) -> Result<librotenado::PartyId, ApiError> {
    let owners = workspace.get(books)?.events().owners();
    owners.first().map(|o| (*o).clone()).ok_or_else(|| ApiError::bad("the books have no owner"))
}

/// The seller's details on invoices; null before they are given.
#[utoipa::path(get, path = "/api/v1/books/{books}/invoicing/profile", params(("books" = String, Path)),
    responses((status = 200, body = Option<dto::InvoiceProfile>)))]
pub(crate) async fn profile(
    State(api): State<Api>,
    Path(books): Path<String>,
) -> Result<Json<Option<dto::InvoiceProfile>>, ApiError> {
    api.run(move |workspace| Ok(Json(workspace.invoice_profile(&books_id(&books)?)?.map(Into::into)))).await
}

/// Keeps the seller's details on invoices.
#[utoipa::path(put, path = "/api/v1/books/{books}/invoicing/profile", params(("books" = String, Path)),
    request_body = dto::InvoiceProfile, responses((status = 200, body = dto::InvoiceProfile)))]
pub(crate) async fn set_profile(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(profile): Json<dto::InvoiceProfile>,
) -> Result<Json<dto::InvoiceProfile>, ApiError> {
    api.run(move |workspace| {
        workspace.set_invoice_profile(&books_id(&books)?, &profile.clone().into())?;
        Ok(Json(profile))
    })
    .await
}

/// The customers, by name.
#[utoipa::path(get, path = "/api/v1/books/{books}/customers", params(("books" = String, Path)),
    responses((status = 200, body = Vec<dto::Customer>)))]
pub(crate) async fn customers(
    State(api): State<Api>,
    Path(books): Path<String>,
) -> Result<Json<Vec<dto::Customer>>, ApiError> {
    api.run(move |workspace| Ok(Json(workspace.customers(&books_id(&books)?)?.into_iter().map(Into::into).collect())))
        .await
}

/// Keeps a customer – new without an identity, else replacing the one with it.
#[utoipa::path(post, path = "/api/v1/books/{books}/customers", params(("books" = String, Path)),
    request_body = dto::Customer, responses((status = 200, body = dto::Saved), (status = 422, body = dto::Problem)))]
pub(crate) async fn save_customer(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(customer): Json<dto::Customer>,
) -> Result<Json<dto::Saved>, ApiError> {
    api.run(move |workspace| Ok(Json(dto::Saved { id: workspace.save_customer(&books_id(&books)?, customer.into())? })))
        .await
}

/// A year.
#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct Year {
    /// The calendar year; this one when none.
    year: Option<u16>,
}

/// The number the next invoice of a year gets.
#[utoipa::path(get, path = "/api/v1/books/{books}/invoices/next-number", params(("books" = String, Path), Year),
    responses((status = 200, body = dto::NextNumber)))]
pub(crate) async fn next_number(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<Year>,
) -> Result<Json<dto::NextNumber>, ApiError> {
    api.run(move |workspace| {
        let year = query.year.unwrap_or_else(|| today().year());
        Ok(Json(dto::NextNumber { number: workspace.next_invoice_number(&books_id(&books)?, year)? }))
    })
    .await
}

/// Writes an invoice as XRechnung and records it; with double entry, its receivable is booked.
#[utoipa::path(post, path = "/api/v1/books/{books}/invoices", params(("books" = String, Path)),
    request_body = dto::NewInvoice, responses((status = 200, body = dto::Issued), (status = 422, body = dto::Problem)))]
pub(crate) async fn issue(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(request): Json<dto::NewInvoice>,
) -> Result<Json<dto::Issued>, ApiError> {
    let id = books.clone();
    let issued = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let owner = owner(workspace, &books)?;
            let date = |text: &str| text.trim().parse().map_err(|_| ApiError::bad(format!("not a date: {text:?}")));
            let currency = "EUR".parse().expect("a currency");
            let lines = request
                .lines
                .iter()
                .map(|line| {
                    Ok(NewLine {
                        name: line.name.trim().to_owned(),
                        quantity: line.quantity.trim().parse().map_err(|_| ApiError::bad("not a quantity"))?,
                        unit: line.unit.clone(),
                        price: parse_amount(&line.price, &currency).map_err(|e| ApiError::bad(e.to_string()))?,
                        tax: line
                            .tax
                            .as_ref()
                            .filter(|t| !t.is_empty())
                            .map(|t| t.parse())
                            .transpose()
                            .map_err(|_| ApiError::bad("not a tax code"))?,
                    })
                })
                .collect::<Result<Vec<_>, ApiError>>()?;
            let invoice = NewInvoice {
                customer: request.customer.clone(),
                date: date(&request.date)?,
                delivery: request.delivery.as_deref().filter(|d| !d.is_empty()).map(date).transpose()?,
                lines,
                note: request.note.clone(),
                exemption: request.exemption.clone(),
                format: match request.format.as_deref() {
                    None | Some("xrechnung") => InvoiceFormat::XRechnung,
                    Some("zugferd") => InvoiceFormat::Zugferd,
                    Some(_) => return Err(ApiError::bad("the format is xrechnung or zugferd")),
                },
            };
            let written = workspace.issue_invoice(&books, &owner, &invoice)?;
            let due = written.document.invoice.as_ref().map(|i| i.totals.due.clone()).expect("an invoice");
            Ok(dto::Issued {
                document: written.document.id.to_string(),
                number: written.document.number.clone().unwrap_or_default(),
                name: written.document.original.name.clone().unwrap_or_default(),
                due: dto::Amount::from(&due),
            })
        })
        .await?;
    api.changed(&id);
    Ok(Json(issued))
}
