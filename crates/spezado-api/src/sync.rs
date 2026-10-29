// SPDX-License-Identifier: AGPL-3.0-or-later
//! The routes of synchronisation: starting it for books, inviting and adding the person's other devices, joining
//! and recovering books, and synchronising now. While `serve` runs, it also synchronises every minute.

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{delete, get, post},
};
use librotenado::BooksId;
use librotenado::Rights;
use spezado_app::SyncReport;

use crate::{Api, ApiError, dto};

/// The routes of synchronisation.
pub fn routes() -> Router<Api> {
    Router::new()
        .route("/books/{books}/sync", get(sync_status).post(sync_now))
        .route("/books/{books}/sync/start", post(sync_start))
        .route("/books/{books}/sync/invitations", post(sync_invite))
        .route("/books/{books}/sync/devices", get(sync_devices))
        .route("/books/{books}/sync/devices/{member}", delete(sync_remove))
        .route("/sync/join", post(sync_join))
        .route("/sync/recover", post(sync_recover))
}

fn books_id(text: &str) -> Result<BooksId, ApiError> {
    text.parse().map_err(|_| ApiError::bad(format!("not the identity of books: {text:?}")))
}

impl From<SyncReport> for dto::SyncReport {
    fn from(report: SyncReport) -> Self {
        dto::SyncReport { received: report.received, sent: report.sent, added: report.added, waiting: report.waiting }
    }
}

/// Whether the books are synchronised, and through which relay.
#[utoipa::path(get, path = "/api/v1/books/{books}/sync", params(("books" = String, Path)),
    responses((status = 200, body = dto::SyncStatus)))]
pub(crate) async fn sync_status(
    State(api): State<Api>,
    Path(books): Path<String>,
) -> Result<Json<dto::SyncStatus>, ApiError> {
    api.run(move |workspace| Ok(Json(dto::SyncStatus { relay: workspace.sync_relay(&books_id(&books)?)? }))).await
}

/// Synchronises the books now: adds devices that asked with an invitation, receives and sends.
#[utoipa::path(post, path = "/api/v1/books/{books}/sync", params(("books" = String, Path)),
    responses((status = 200, body = dto::SyncReport), (status = 422, body = dto::Problem)))]
pub(crate) async fn sync_now(
    State(api): State<Api>,
    Path(books): Path<String>,
) -> Result<Json<dto::SyncReport>, ApiError> {
    let id = books.clone();
    let report = api.run(move |workspace| Ok(workspace.sync(&books_id(&books)?)?)).await?;
    if report.received > 0 {
        api.changed(&id);
    }
    Ok(Json(report.into()))
}

/// Starts synchronising the books through a relay; answers with the recovery code, shown this once.
#[utoipa::path(post, path = "/api/v1/books/{books}/sync/start", params(("books" = String, Path)),
    request_body = dto::SyncStart, responses((status = 200, body = dto::RecoveryCode), (status = 422, body = dto::Problem)))]
pub(crate) async fn sync_start(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(request): Json<dto::SyncStart>,
) -> Result<Json<dto::RecoveryCode>, ApiError> {
    api.run(move |workspace| {
        let code = workspace.start_sync(&books_id(&books)?, request.relay.trim(), request.name.as_deref())?;
        Ok(Json(dto::RecoveryCode { code: code.to_string() }))
    })
    .await
}

/// An invitation for another device, good for a day: the person's own (`manage`), or someone else's with the
/// rights given.
#[utoipa::path(post, path = "/api/v1/books/{books}/sync/invitations", params(("books" = String, Path)),
    request_body = dto::SyncInvite, responses((status = 200, body = dto::SyncInvitation), (status = 422, body = dto::Problem)))]
pub(crate) async fn sync_invite(
    State(api): State<Api>,
    Path(books): Path<String>,
    body: Option<Json<dto::SyncInvite>>,
) -> Result<Json<dto::SyncInvitation>, ApiError> {
    let rights = rights(body.and_then(|Json(b)| b.rights).as_deref().unwrap_or("manage"))?;
    api.run(move |workspace| {
        Ok(Json(dto::SyncInvitation { invitation: workspace.invite(&books_id(&books)?, rights)? }))
    })
    .await
}

fn rights(text: &str) -> Result<Rights, ApiError> {
    match text {
        "read" => Ok(Rights::Read),
        "write" => Ok(Rights::Write),
        "manage" => Ok(Rights::Manage),
        _ => Err(ApiError::bad("rights are read, write or manage")),
    }
}

fn rights_text(rights: Rights) -> &'static str {
    match rights {
        Rights::Read => "read",
        Rights::Write => "write",
        Rights::Manage => "manage",
    }
}

/// Removes a device or person from the books, by identity, device name or name; the key changes.
#[utoipa::path(delete, path = "/api/v1/books/{books}/sync/devices/{member}",
    params(("books" = String, Path), ("member" = String, Path)),
    responses((status = 200, body = dto::Removed), (status = 422, body = dto::Problem)))]
pub(crate) async fn sync_remove(
    State(api): State<Api>,
    Path((books, member)): Path<(String, String)>,
) -> Result<Json<dto::Removed>, ApiError> {
    api.run(move |workspace| Ok(Json(dto::Removed { name: workspace.remove_member(&books_id(&books)?, &member)? })))
        .await
}

/// The devices of the books, as their relay lists them.
#[utoipa::path(get, path = "/api/v1/books/{books}/sync/devices", params(("books" = String, Path)),
    responses((status = 200, body = Vec<dto::SyncDevice>), (status = 422, body = dto::Problem)))]
pub(crate) async fn sync_devices(
    State(api): State<Api>,
    Path(books): Path<String>,
) -> Result<Json<Vec<dto::SyncDevice>>, ApiError> {
    api.run(move |workspace| {
        let members = workspace.sync_members(&books_id(&books)?)?;
        Ok(Json(
            members
                .into_iter()
                .map(|m| dto::SyncDevice {
                    id: m.id,
                    name: m.name,
                    device: m.device,
                    rights: rights_text(m.rights).into(),
                    recovery: m.recovery,
                    me: m.me,
                })
                .collect(),
        ))
    })
    .await
}

/// Asks to join books with an invitation from another device; they arrive once that device has synchronised.
#[utoipa::path(post, path = "/api/v1/sync/join", request_body = dto::SyncJoin,
    responses((status = 200, body = dto::BooksRef), (status = 422, body = dto::Problem)))]
pub(crate) async fn sync_join(
    State(api): State<Api>,
    Json(request): Json<dto::SyncJoin>,
) -> Result<Json<dto::BooksRef>, ApiError> {
    api.run(move |workspace| {
        let books = workspace.join(request.invitation.trim(), request.name.as_deref())?;
        Ok(Json(dto::BooksRef { books: books.to_string() }))
    })
    .await
}

/// Brings books back to this device with their recovery code.
#[utoipa::path(post, path = "/api/v1/sync/recover", request_body = dto::SyncRecover,
    responses((status = 200, body = dto::SyncReport), (status = 422, body = dto::Problem)))]
pub(crate) async fn sync_recover(
    State(api): State<Api>,
    Json(request): Json<dto::SyncRecover>,
) -> Result<Json<dto::SyncReport>, ApiError> {
    let id = request.books.clone();
    let report = api
        .run(move |workspace| {
            let code = request.code.parse().map_err(|_| ApiError::bad("not a valid recovery code"))?;
            Ok(workspace.recover(request.relay.trim(), &books_id(&request.books)?, &code, request.name.as_deref())?)
        })
        .await?;
    api.changed(&id);
    Ok(Json(report.into()))
}

/// Synchronises every set of books that is, every minute, and says which changed.
pub(crate) async fn every_minute(api: Api) {
    let mut ticks = tokio::time::interval(std::time::Duration::from_secs(60));
    loop {
        ticks.tick().await;
        let reports = api.run(|workspace| Ok(workspace.sync_all().unwrap_or_default())).await.unwrap_or_default();
        for (books, report) in reports {
            if report.received > 0 {
                api.changed(books.as_str());
            }
        }
    }
}
