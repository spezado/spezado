// SPDX-License-Identifier: AGPL-3.0-or-later
//! The routes of fixed assets: the register, a new asset, a disposal, and the depreciation of a year.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use librotenado::{BooksId, Depreciation};
use serde::Deserialize;
use spezado_app::{NewAsset, Workspace, today};
use spezado_core::parse_amount;
use utoipa::IntoParams;

use crate::{Api, ApiError, dto};

/// The routes of fixed assets.
pub fn routes() -> Router<Api> {
    Router::new()
        .route("/books/{books}/assets", get(assets).post(register))
        .route("/books/{books}/assets/{asset}/disposal", post(dispose))
        .route("/books/{books}/depreciation", post(depreciate))
}

fn books_id(text: &str) -> Result<BooksId, ApiError> {
    text.parse().map_err(|_| ApiError::bad(format!("not the identity of books: {text:?}")))
}

fn owner(workspace: &Workspace, books: &BooksId) -> Result<librotenado::PartyId, ApiError> {
    let owners = workspace.get(books)?.events().owners();
    owners.first().map(|o| (*o).clone()).ok_or_else(|| ApiError::bad("the books have no owner"))
}

fn date(text: &str) -> Result<librotenado::Date, ApiError> {
    text.parse().map_err(|_| ApiError::bad(format!("not a date YYYY-MM-DD: {text:?}")))
}

/// The day the book values are given at.
#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct AtQuery {
    /// `YYYY-MM-DD`; default: today.
    at: Option<String>,
}

/// The fixed assets of the owner, with their book values at a day.
#[utoipa::path(get, path = "/api/v1/books/{books}/assets", params(("books" = String, Path), AtQuery),
    responses((status = 200, body = Vec<dto::AssetView>)))]
pub(crate) async fn assets(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<AtQuery>,
) -> Result<Json<Vec<dto::AssetView>>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let at = query.at.as_deref().map(date).transpose()?.unwrap_or_else(today);
        let national = workspace.national_chart(&books, &owner, at)?;
        let items = workspace.assets(&books, &owner, at)?;
        Ok(Json(
            items
                .into_iter()
                .map(|item| {
                    let asset = &item.state.asset;
                    let currency = asset.cost.currency().clone();
                    let (method, months) = match asset.depreciation {
                        Depreciation::StraightLine { months } => ("straight_line", Some(months)),
                        Depreciation::Immediate => ("immediate", None),
                        Depreciation::Pool => ("pool", None),
                        Depreciation::None => ("none", None),
                    };
                    dto::AssetView {
                        id: asset.id.to_string(),
                        name: asset.name.clone(),
                        account: national
                            .as_ref()
                            .and_then(|n| n.national(&asset.account))
                            .map_or_else(|| asset.account.to_string(), str::to_owned),
                        acquired: asset.acquired.to_string(),
                        cost: dto::Amount::from(&asset.cost),
                        method: method.into(),
                        months,
                        disposed: item.state.disposal.as_ref().map(|d| d.date.to_string()),
                        book_value: dto::Amount::from(
                            &librotenado::Money::new(item.book_value, currency).expect("within the cost"),
                        ),
                    }
                })
                .collect(),
        ))
    })
    .await
}

/// Registers a fixed asset.
#[utoipa::path(post, path = "/api/v1/books/{books}/assets", params(("books" = String, Path)),
    request_body = dto::NewAsset, responses((status = 200, body = dto::Saved), (status = 422, body = dto::Problem)))]
pub(crate) async fn register(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(request): Json<dto::NewAsset>,
) -> Result<Json<dto::Saved>, ApiError> {
    let id = books.clone();
    let saved = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let owner = owner(workspace, &books)?;
            let account = crate::routes::account(workspace, &books, &owner, &request.account)?;
            let cost = parse_amount(&request.cost, &"EUR".parse().expect("a currency"))
                .map_err(|e| ApiError::bad(e.to_string()))?;
            let depreciation = match (request.method.as_str(), request.months) {
                ("straight_line", Some(months)) => Depreciation::StraightLine { months },
                ("immediate", _) => Depreciation::Immediate,
                ("pool", _) => Depreciation::Pool,
                ("none", _) => Depreciation::None,
                (other, _) => {
                    return Err(ApiError::bad(format!(
                        "method is straight_line with months, immediate, pool or none, not {other:?}"
                    )));
                }
            };
            let new =
                NewAsset { owner, name: request.name, account, acquired: date(&request.acquired)?, cost, depreciation };
            Ok(dto::Saved { id: workspace.register_asset(&books, new)?.to_string() })
        })
        .await?;
    api.changed(&id);
    Ok(Json(saved))
}

/// Records that a fixed asset left the business and books its book value out.
#[utoipa::path(post, path = "/api/v1/books/{books}/assets/{asset}/disposal",
    params(("books" = String, Path), ("asset" = String, Path)), request_body = dto::AssetDisposal,
    responses((status = 200), (status = 422, body = dto::Problem)))]
pub(crate) async fn dispose(
    State(api): State<Api>,
    Path((books, asset)): Path<(String, String)>,
    Json(request): Json<dto::AssetDisposal>,
) -> Result<Json<()>, ApiError> {
    let id = books.clone();
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let asset = asset.parse().map_err(|_| ApiError::bad("not the identity of an asset"))?;
        let proceeds = request
            .proceeds
            .as_deref()
            .filter(|p| !p.is_empty())
            .map(|p| parse_amount(p, &"EUR".parse().expect("a currency")))
            .transpose()
            .map_err(|e| ApiError::bad(e.to_string()))?;
        workspace.dispose_asset(&books, &asset, date(&request.date)?, proceeds)?;
        Ok(())
    })
    .await?;
    api.changed(&id);
    Ok(Json(()))
}

/// Books the owner's depreciation of a calendar year, once; answers whether there was anything to book.
#[utoipa::path(post, path = "/api/v1/books/{books}/depreciation", params(("books" = String, Path)),
    request_body = dto::DepreciationYear, responses((status = 200, body = bool), (status = 422, body = dto::Problem)))]
pub(crate) async fn depreciate(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(request): Json<dto::DepreciationYear>,
) -> Result<Json<bool>, ApiError> {
    let id = books.clone();
    let booked = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let owner = owner(workspace, &books)?;
            Ok(workspace.book_depreciation(&books, &owner, request.year)?)
        })
        .await?;
    api.changed(&id);
    Ok(Json(booked))
}
