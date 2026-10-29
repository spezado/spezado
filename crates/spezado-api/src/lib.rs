// SPDX-License-Identifier: AGPL-3.0-or-later
//! The open API of spezado: HTTP and JSON on the device itself, described in OpenAPI, for the web app and the apps.
//! Resources are read, workflows and commands written; Server-Sent Events say when something changed. A device pairs
//! once with a code `spezado serve` shows and gets a token of its own, which can be revoked; tokens are stored as
//! their hash. Like every interface, it works through the workspace of `spezado-app` only.

use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spezado_app::{AppError, Workspace, new_id, today};
use spezado_core::CommandError;
use tokio::sync::broadcast;

pub mod dto;
mod invoices;
mod records;
mod routes;
mod sync;
mod tls;

pub use routes::ApiDoc;

/// The description of the API in OpenAPI, as pretty JSON – what `api/openapi.json` holds.
pub fn utoipa_openapi() -> String {
    use utoipa::OpenApi;
    ApiDoc::openapi().to_pretty_json().expect("the description in JSON")
}

/// Why the server could not run.
#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    /// The network or the runtime.
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

/// An answer that went wrong: a status and what to say.
#[derive(Debug)]
pub struct ApiError(StatusCode, String);

impl ApiError {
    /// A request that cannot be served as asked.
    pub fn bad(message: impl Into<String>) -> Self {
        ApiError(StatusCode::BAD_REQUEST, message.into())
    }

    fn not_found(message: impl Into<String>) -> Self {
        ApiError(StatusCode::NOT_FOUND, message.into())
    }

    fn unauthorised() -> Self {
        ApiError(StatusCode::UNAUTHORIZED, "pair this device first: POST /api/v1/pair".into())
    }
}

impl From<AppError> for ApiError {
    fn from(error: AppError) -> Self {
        let status = match &error {
            AppError::UnknownBooks(_) | AppError::UnknownReport(_) => StatusCode::NOT_FOUND,
            AppError::Command(CommandError::NotFounded) => StatusCode::NOT_FOUND,
            AppError::Store(_) | AppError::Write { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::FinTsNotRegistered | AppError::FinTsNotBuilt => StatusCode::NOT_IMPLEMENTED,
            _ => StatusCode::UNPROCESSABLE_ENTITY,
        };
        ApiError(status, error.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(dto::Problem { error: self.1 })).into_response()
    }
}

/// A device paired with the API, as stored in the workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeviceRecord {
    id: String,
    name: String,
    hash: String,
    paired: String,
}

/// Where the paired devices are kept in the workspace.
const DEVICES: &str = "api-devices";

/// How long a pairing code is valid.
const PAIRING: Duration = Duration::from_secs(15 * 60);

/// The state of the server: the workspace, the code to pair with, and the stream of changes.
#[derive(Debug, Clone)]
pub struct Api {
    workspace: Arc<Mutex<Workspace>>,
    pairing: Arc<Mutex<Option<(String, Instant)>>>,
    changes: broadcast::Sender<dto::Change>,
    pending: Arc<Mutex<records::Pending>>,
}

/// Random hexadecimal digits from the operating system.
fn random_hex(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    getrandom::fill(&mut buffer).expect("the operating system's randomness");
    hex::encode(buffer)
}

fn hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

impl Api {
    /// The API on a workspace.
    pub fn new(workspace: Workspace) -> Self {
        let (changes, _) = broadcast::channel(64);
        Api {
            workspace: Arc::new(Mutex::new(workspace)),
            pairing: Arc::new(Mutex::new(None)),
            changes,
            pending: Arc::new(Mutex::new(Default::default())),
        }
    }

    /// A new code to pair a device with, valid for a quarter of an hour; the one before stops being valid.
    pub fn pairing_code(&self) -> String {
        let code = random_hex(5);
        self.set_pairing_code(&code);
        code
    }

    /// A code to pair a device with, as given – by an app that starts spezado and pairs itself; valid like one made
    /// up, and the one before stops being valid.
    pub fn set_pairing_code(&self, code: &str) {
        *self.pairing.lock().expect("the pairing code") = Some((code.to_owned(), Instant::now() + PAIRING));
    }

    /// Runs `work` on the workspace, away from the server's threads.
    async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce(&mut Workspace) -> Result<T, ApiError> + Send + 'static,
    ) -> Result<T, ApiError> {
        let workspace = self.workspace.clone();
        tokio::task::spawn_blocking(move || {
            let mut workspace = workspace.lock().map_err(|_| ApiError::bad("the workspace failed earlier"))?;
            work(&mut workspace)
        })
        .await
        .map_err(|error| ApiError(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
    }

    /// Says that a set of books changed.
    fn changed(&self, books: &str) {
        let _ = self.changes.send(dto::Change { books: books.to_owned() });
    }

    fn devices(workspace: &Workspace) -> Result<Vec<DeviceRecord>, ApiError> {
        let stored = workspace.workspace_setting(DEVICES)?.unwrap_or_else(|| "[]".into());
        serde_json::from_str(&stored).map_err(|e| ApiError::bad(e.to_string()))
    }

    fn store_devices(workspace: &Workspace, devices: &[DeviceRecord]) -> Result<(), ApiError> {
        let json = serde_json::to_string(devices).map_err(|e| ApiError::bad(e.to_string()))?;
        Ok(workspace.set_workspace_setting(DEVICES, &json)?)
    }

    /// Exchanges the pairing code for a token of a new device; the code is used up.
    async fn pair(&self, request: dto::PairRequest) -> Result<dto::Paired, ApiError> {
        {
            let mut pairing = self.pairing.lock().expect("the pairing code");
            match pairing.as_ref() {
                Some((code, until)) if *code == request.code.trim().to_lowercase() && Instant::now() < *until => {
                    *pairing = None;
                }
                _ => return Err(ApiError(StatusCode::FORBIDDEN, "the code is not valid".into())),
            }
        }
        let token = random_hex(32);
        let device = DeviceRecord {
            id: new_id::<librotenado::DocumentId>().to_string(),
            name: request.name.trim().chars().take(80).collect(),
            hash: hash(&token),
            paired: today().to_string(),
        };
        let id = device.id.clone();
        self.run(move |workspace| {
            let mut devices = Api::devices(workspace)?;
            devices.push(device);
            Api::store_devices(workspace, &devices)
        })
        .await?;
        Ok(dto::Paired { token, device: id })
    }

    /// Whether a token is one of a paired device.
    async fn authorised(&self, token: String) -> bool {
        let hash = hash(&token);
        self.run(move |workspace| Ok(Api::devices(workspace)?.iter().any(|d| d.hash == hash))).await.unwrap_or(false)
    }
}

/// Lets through only requests with the token of a paired device – as `Authorization: Bearer`, or, for the stream of
/// events a browser opens without headers, as the query `token`.
async fn authorise(State(api): State<Api>, headers: HeaderMap, request: Request, next: Next) -> Response {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_owned);
    let query = request
        .uri()
        .query()
        .and_then(|query| query.split('&').find_map(|pair| pair.strip_prefix("token=")).map(str::to_owned));
    let path = request.uri().path().to_owned();
    let token = bearer.or(if path.ends_with("/events") { query } else { None });
    match token {
        Some(token) if api.authorised(token.clone()).await => next.run(request).await,
        _ => ApiError::unauthorised().into_response(),
    }
}

/// The web app, built by `pnpm build` in `web/` and embedded into the binary; a build without it serves the API alone.
#[derive(rust_embed::Embed)]
#[folder = "../../web/dist"]
#[allow_missing = true]
struct WebApp;

/// What the web app may load: itself only – no script, style, font or connection elsewhere, no frames.
const POLICY: &str = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; \
font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";

/// A file of the web app; any other path is the app itself, which routes by the address.
async fn web_app(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let (file, path) = match WebApp::get(path) {
        Some(file) if !path.is_empty() => (file, path),
        _ => match WebApp::get("index.html") {
            Some(file) => (file, "index.html"),
            None => return Html(NO_WEB_APP).into_response(),
        },
    };
    // Built files carry their hash in their name and never change; the page itself is asked for every time.
    let cache = if path.starts_with("assets/") { "public, max-age=31536000, immutable" } else { "no-cache" };
    (
        [
            (header::CONTENT_TYPE, file.metadata.mimetype().to_owned()),
            (header::CACHE_CONTROL, cache.to_owned()),
            (header::CONTENT_SECURITY_POLICY, POLICY.to_owned()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
            (header::REFERRER_POLICY, "no-referrer".to_owned()),
        ],
        file.data,
    )
        .into_response()
}

/// The page shown where the web app is not built into the binary.
const NO_WEB_APP: &str = "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>spezado</title>\
<body style=\"font-family:system-ui;max-width:40rem;margin:4rem auto;padding:0 1rem\"><h1>spezado</h1>\
<p>The API runs at <code>/api/v1</code>, described at <code>/api/v1/openapi.json</code>. This build has no web app.</p>";

/// The router: the API under `/api/v1`, and the web app around it.
pub fn router(api: Api) -> Router {
    let open = routes::open();
    // Files of up to 32 MB: invoices, photos, statements.
    let protected = routes::protected()
        .merge(records::routes())
        .merge(invoices::routes())
        .merge(sync::routes())
        .layer(axum::extract::DefaultBodyLimit::max(32 * 1024 * 1024))
        .route_layer(middleware::from_fn_with_state(api.clone(), authorise));
    Router::new()
        .nest("/api/v1", open.merge(protected).fallback(|| async { ApiError::not_found("no such route of the API") }))
        .fallback(web_app)
        .with_state(api)
}

/// Where `serve` listens and what it tells: the address to open, the pairing code, and – with TLS – the fingerprint
/// of the certificate.
#[derive(Debug, Clone)]
pub struct Serving {
    /// The address to open, with the pairing code: `http://127.0.0.1:8048/#pair=…`, or with TLS
    /// `https://host:8443/#pair=…&fingerprint=sha256:…`.
    pub link: String,
    /// The pairing code.
    pub code: String,
    /// The fingerprint of the certificate, with TLS.
    pub fingerprint: Option<String>,
}

/// Serves the API and the web app for a workspace at `listen` until interrupted. On the device itself (a loopback
/// address) it speaks HTTP; to other devices only TLS, with a certificate of the workspace's own made for `host` –
/// the name other devices reach this one by. The pairing code is `pairing` when given, else made up. `ready` is told
/// what to show.
pub fn serve(
    workspace: Workspace,
    listen: SocketAddr,
    host: &str,
    pairing: Option<String>,
    ready: impl FnOnce(&Serving),
) -> Result<(), ServeError> {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let tls = if listen.ip().is_loopback() { None } else { Some(tls::configuration(&workspace, host)?) };
    runtime.block_on(async move {
        let api = Api::new(workspace);
        let code = match pairing {
            Some(code) => {
                api.set_pairing_code(&code);
                code
            }
            None => api.pairing_code(),
        };
        tokio::spawn(sync::every_minute(api.clone()));
        match tls {
            None => {
                let listener = tokio::net::TcpListener::bind(listen).await?;
                let address = listener.local_addr()?;
                ready(&Serving { link: format!("http://{address}/#pair={code}"), code, fingerprint: None });
                axum::serve(listener, router(api))
                    .with_graceful_shutdown(async {
                        let _ = tokio::signal::ctrl_c().await;
                    })
                    .await?;
            }
            Some((config, fingerprint)) => {
                let handle = axum_server::Handle::new();
                let stop = handle.clone();
                tokio::spawn(async move {
                    let _ = tokio::signal::ctrl_c().await;
                    stop.graceful_shutdown(Some(std::time::Duration::from_secs(5)));
                });
                let server =
                    axum_server::bind_rustls(listen, axum_server::tls_rustls::RustlsConfig::from_config(config))
                        .handle(handle.clone());
                let serving = tokio::spawn(server.serve(router(api).into_make_service()));
                let address = handle.listening().await.unwrap_or(listen);
                ready(&Serving {
                    link: format!("https://{host}:{}/#pair={code}&fingerprint={fingerprint}", address.port()),
                    code,
                    fingerprint: Some(fingerprint),
                });
                serving.await.map_err(std::io::Error::other)??;
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests;
