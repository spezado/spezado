// SPDX-License-Identifier: AGPL-3.0-or-later
//! The relay of spezado (librotenado part 0006): it holds, per set of books, the ordered member list and the
//! envelopes of events and originals, and passes them on to the members. It cannot read them: it checks signatures,
//! the rules of the member list and who may write, never content.
//!
//! The relay speaks HTTP; it is meant to run behind a reverse proxy that terminates TLS.

use std::{
    net::SocketAddr,
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path as UrlPath, Query, State},
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
};
use librotenado::{
    BooksId, Envelope, JoinRequest, Member, MemberId, MemberList, MemberRecord, MemberRole, RecordProblem, Rights,
    from_root, to_root, verify_request,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

/// The schema of the relay's database.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS records (
    books TEXT NOT NULL,
    seq INTEGER NOT NULL,
    canonical BLOB NOT NULL,
    PRIMARY KEY (books, seq)
) STRICT;
CREATE TABLE IF NOT EXISTS envelopes (
    books TEXT NOT NULL,
    position INTEGER NOT NULL,
    id TEXT NOT NULL,
    canonical BLOB NOT NULL,
    PRIMARY KEY (books, position),
    UNIQUE (books, id)
) STRICT;
CREATE TABLE IF NOT EXISTS requests (
    books TEXT NOT NULL,
    member TEXT NOT NULL,
    canonical BLOB NOT NULL,
    received INTEGER NOT NULL,
    PRIMARY KEY (books, member)
) STRICT;
PRAGMA user_version = 1;
";

/// How long a request to join is kept, in seconds.
const REQUEST_LIFETIME: u64 = 24 * 60 * 60;
/// How many requests to join a set of books may wait at once.
const MAX_REQUESTS: i64 = 20;
/// The largest request body: an original in an envelope.
const MAX_BODY: usize = 64 * 1024 * 1024;

/// The state of a relay: its database.
#[derive(Debug, Clone)]
pub struct Relay {
    connection: Arc<Mutex<Connection>>,
}

/// An error answered to a request, as `{"error": …}`.
#[derive(Debug)]
pub struct RelayError(StatusCode, String);

impl IntoResponse for RelayError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}

impl From<rusqlite::Error> for RelayError {
    fn from(error: rusqlite::Error) -> Self {
        RelayError(StatusCode::INTERNAL_SERVER_ERROR, format!("database: {error}"))
    }
}

fn bad(message: impl Into<String>) -> RelayError {
    RelayError(StatusCode::BAD_REQUEST, message.into())
}

fn forbidden(message: impl Into<String>) -> RelayError {
    RelayError(StatusCode::FORBIDDEN, message.into())
}

/// Seconds since 1970-01-01 UTC.
fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

fn books_id(text: &str) -> Result<BooksId, RelayError> {
    text.parse().map_err(|_| bad("not the identity of books"))
}

impl Relay {
    /// Opens the relay's database in `directory`, creating it if needed.
    pub fn open(directory: &Path) -> Result<Self, rusqlite::Error> {
        std::fs::create_dir_all(directory).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let connection = Connection::open(directory.join("relay.sqlite"))?;
        connection.execute_batch("PRAGMA journal_mode = WAL;")?;
        connection.execute_batch(SCHEMA)?;
        Ok(Relay { connection: Arc::new(Mutex::new(connection)) })
    }

    /// The member list of a set of books, as stored.
    fn members(connection: &Connection, books: &BooksId) -> Result<MemberList, RelayError> {
        let mut statement = connection.prepare("SELECT canonical FROM records WHERE books = ?1 ORDER BY seq")?;
        let rows = statement.query_map(params![books.as_str()], |row| row.get::<_, Vec<u8>>(0))?;
        let mut list = MemberList::new(books.clone());
        for row in rows {
            let record: MemberRecord = from_root(&row?).map_err(|e| bad(e.to_string()))?;
            list.apply(record).map_err(|e| RelayError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
        Ok(list)
    }

    /// The member who signed a request, if it is a current member of the books with at least `rights`.
    fn member(
        &self,
        books: &BooksId,
        headers: &HeaderMap,
        method: &Method,
        uri: &Uri,
        rights: Rights,
    ) -> Result<(MemberList, Member), RelayError> {
        let header = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| RelayError(StatusCode::UNAUTHORIZED, "a signed request is needed".into()))?;
        let path = uri.path_and_query().map_or(uri.path(), |p| p.as_str());
        let id = verify_request(header, method.as_str(), path, now())
            .ok_or_else(|| RelayError(StatusCode::UNAUTHORIZED, "the signature or its time is not valid".into()))?;
        let connection = self.connection.lock().expect("the relay's database");
        let list = Self::members(&connection, books)?;
        let member = list.member(&id).cloned().ok_or_else(|| forbidden("not a member of these books"))?;
        if member.rights < rights {
            return Err(forbidden("not allowed"));
        }
        Ok((list, member))
    }
}

/// The routes of the relay (librotenado part 0006, relays).
pub fn router(relay: Relay) -> Router {
    Router::new()
        .route("/v1/books/{books}/members", get(members).post(add_record))
        .route("/v1/books/{books}/envelopes", get(envelopes).post(add_envelope))
        .route("/v1/books/{books}/requests", get(requests).post(add_request))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .with_state(relay)
}

#[derive(Debug, Deserialize)]
struct After {
    after: Option<i64>,
    limit: Option<i64>,
}

/// The records after `after`, in order – open.
async fn members(
    State(relay): State<Relay>,
    UrlPath(books): UrlPath<String>,
    Query(query): Query<After>,
) -> Result<Json<Vec<Value>>, RelayError> {
    let books = books_id(&books)?;
    let connection = relay.connection.lock().expect("the relay's database");
    let mut statement =
        connection.prepare("SELECT canonical FROM records WHERE books = ?1 AND seq > ?2 ORDER BY seq")?;
    let rows =
        statement.query_map(params![books.as_str(), query.after.unwrap_or(-1)], |row| row.get::<_, Vec<u8>>(0))?;
    let records = rows
        .map(|row| Ok(serde_json::from_slice(&row?).expect("a stored record is JSON")))
        .collect::<Result<Vec<Value>, rusqlite::Error>>()?;
    Ok(Json(records))
}

/// A record of the member list: the next valid one, or refused.
async fn add_record(
    State(relay): State<Relay>,
    UrlPath(books): UrlPath<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<StatusCode, RelayError> {
    let books = books_id(&books)?;
    let record: MemberRecord = from_root(&body).map_err(|e| bad(e.to_string()))?;
    if record.seq > 0 {
        relay.member(&books, &headers, &method, &uri, Rights::Manage)?;
    }
    let connection = relay.connection.lock().expect("the relay's database");
    let mut list = Relay::members(&connection, &books)?;
    let seq = record.seq;
    let added: Vec<MemberId> = record.add.iter().map(|m| m.id).collect();
    let canonical = to_root(&record).expect("a record has a canonical encoding");
    match list.apply(record) {
        Ok(()) => {}
        Err(RecordProblem::NotNext) => return Err(RelayError(StatusCode::CONFLICT, "not the next record".into())),
        Err(problem) => return Err(forbidden(problem.to_string())),
    }
    connection.execute(
        "INSERT INTO records (books, seq, canonical) VALUES (?1, ?2, ?3)",
        params![books.as_str(), seq as i64, canonical],
    )?;
    for id in added {
        connection.execute(
            "DELETE FROM requests WHERE books = ?1 AND member = ?2",
            params![books.as_str(), id.to_string()],
        )?;
    }
    Ok(StatusCode::CREATED)
}

/// An envelope from a member who may write.
async fn add_envelope(
    State(relay): State<Relay>,
    UrlPath(books): UrlPath<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<StatusCode, RelayError> {
    let books = books_id(&books)?;
    let (list, _) = relay.member(&books, &headers, &method, &uri, Rights::Write)?;
    let envelope: Envelope = from_root(&body).map_err(|e| bad(e.to_string()))?;
    let author = list.member(&envelope.author).ok_or_else(|| forbidden("the author is not a member"))?;
    let writes = author.rights >= Rights::Write && author.role != MemberRole::Recovery;
    if envelope.books != books || !writes || !envelope.is_authentic() {
        return Err(forbidden("the envelope is not one a member may send"));
    }
    if list.key_index().is_none_or(|current| envelope.key > current) {
        return Err(forbidden("no such key yet"));
    }
    let connection = relay.connection.lock().expect("the relay's database");
    let known: Option<i64> = connection
        .query_row(
            "SELECT position FROM envelopes WHERE books = ?1 AND id = ?2",
            params![books.as_str(), envelope.id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    if known.is_some() {
        return Ok(StatusCode::OK);
    }
    let position: i64 = connection.query_row(
        "SELECT COALESCE(MAX(position), 0) + 1 FROM envelopes WHERE books = ?1",
        params![books.as_str()],
        |row| row.get(0),
    )?;
    connection.execute(
        "INSERT INTO envelopes (books, position, id, canonical) VALUES (?1, ?2, ?3, ?4)",
        params![books.as_str(), position, envelope.id.to_string(), to_root(&envelope).expect("canonical")],
    )?;
    Ok(StatusCode::CREATED)
}

/// The envelopes after a position, for members.
async fn envelopes(
    State(relay): State<Relay>,
    UrlPath(books): UrlPath<String>,
    Query(query): Query<After>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<Value>, RelayError> {
    let books = books_id(&books)?;
    relay.member(&books, &headers, &method, &uri, Rights::Read)?;
    let limit = query.limit.unwrap_or(100).clamp(1, 1000);
    let connection = relay.connection.lock().expect("the relay's database");
    let mut statement = connection.prepare(
        "SELECT position, canonical FROM envelopes WHERE books = ?1 AND position > ?2 ORDER BY position LIMIT ?3",
    )?;
    let rows = statement.query_map(params![books.as_str(), query.after.unwrap_or(0), limit + 1], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;
    let mut envelopes = rows
        .map(|row| {
            let (position, canonical) = row?;
            let envelope: Value = serde_json::from_slice(&canonical).expect("a stored envelope is JSON");
            Ok(json!({"position": position, "envelope": envelope}))
        })
        .collect::<Result<Vec<Value>, rusqlite::Error>>()?;
    let more = envelopes.len() as i64 > limit;
    envelopes.truncate(limit as usize);
    Ok(Json(json!({"envelopes": envelopes, "more": more})))
}

/// A request to join – open; the inviting member checks its proof.
async fn add_request(
    State(relay): State<Relay>,
    UrlPath(books): UrlPath<String>,
    body: axum::body::Bytes,
) -> Result<StatusCode, RelayError> {
    let books = books_id(&books)?;
    let request: JoinRequest = from_root(&body).map_err(|e| bad(e.to_string()))?;
    let connection = relay.connection.lock().expect("the relay's database");
    let held: i64 =
        connection.query_row("SELECT COUNT(*) FROM records WHERE books = ?1", params![books.as_str()], |r| r.get(0))?;
    if held == 0 {
        return Err(RelayError(StatusCode::NOT_FOUND, "no such books here".into()));
    }
    connection
        .execute("DELETE FROM requests WHERE received < ?1", params![now().saturating_sub(REQUEST_LIFETIME) as i64])?;
    let waiting: i64 =
        connection
            .query_row("SELECT COUNT(*) FROM requests WHERE books = ?1", params![books.as_str()], |r| r.get(0))?;
    if waiting >= MAX_REQUESTS {
        return Err(RelayError(StatusCode::TOO_MANY_REQUESTS, "too many requests waiting".into()));
    }
    connection.execute(
        "INSERT OR REPLACE INTO requests (books, member, canonical, received) VALUES (?1, ?2, ?3, ?4)",
        params![books.as_str(), request.member.id.to_string(), to_root(&request).expect("canonical"), now() as i64],
    )?;
    Ok(StatusCode::CREATED)
}

/// The requests to join, for members who may manage.
async fn requests(
    State(relay): State<Relay>,
    UrlPath(books): UrlPath<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<Vec<Value>>, RelayError> {
    let books = books_id(&books)?;
    relay.member(&books, &headers, &method, &uri, Rights::Manage)?;
    let connection = relay.connection.lock().expect("the relay's database");
    let mut statement =
        connection.prepare("SELECT canonical FROM requests WHERE books = ?1 AND received >= ?2 ORDER BY received")?;
    let since = now().saturating_sub(REQUEST_LIFETIME) as i64;
    let rows = statement.query_map(params![books.as_str(), since], |row| row.get::<_, Vec<u8>>(0))?;
    let requests = rows
        .map(|row| Ok(serde_json::from_slice(&row?).expect("a stored request is JSON")))
        .collect::<Result<Vec<Value>, rusqlite::Error>>()?;
    Ok(Json(requests))
}

/// Opens the relay in `directory` and serves it at `address` until Ctrl-C, on a runtime of its own.
pub fn run(directory: &Path, address: SocketAddr, started: impl FnOnce(SocketAddr)) -> std::io::Result<()> {
    let relay = Relay::open(directory).map_err(std::io::Error::other)?;
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(address).await?;
        started(listener.local_addr()?);
        axum::serve(listener, router(relay))
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
    })
}

/// Serves the relay at `address` until Ctrl-C.
pub async fn serve(relay: Relay, address: SocketAddr) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, router(relay))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}
