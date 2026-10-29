// SPDX-License-Identifier: AGPL-3.0-or-later
//! The API through its router, without a network: pairing, a freelancer's bookings, reports, changes, its description.

use std::{fs, path::PathBuf};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use spezado_app::Workspace;
use tower::ServiceExt;

use crate::{Api, router, utoipa_openapi};

/// A temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("spezado-api-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Sends a request; returns the status and the body as JSON (null when empty).
async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = match body {
        Some(body) => request.header(header::CONTENT_TYPE, "application/json").body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

fn setup() -> (Scratch, Api, Router) {
    let scratch = Scratch::new(&format!("{:?}", std::thread::current().id()).replace(['(', ')'], ""));
    let workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let api = Api::new(workspace);
    let app = router(api.clone());
    (scratch, api, app)
}

async fn paired(api: &Api, app: &Router) -> String {
    let code = api.pairing_code();
    let (status, body) =
        send(app, Method::POST, "/api/v1/pair", None, Some(json!({"code": code, "name": "test"}))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["token"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn only_paired_devices_get_in() {
    let (_scratch, api, app) = setup();
    assert_eq!(send(&app, Method::GET, "/api/v1/books", None, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(send(&app, Method::GET, "/api/v1/books", Some("guessed"), None).await.0, StatusCode::UNAUTHORIZED);
    let code = api.pairing_code();
    let wrong = send(&app, Method::POST, "/api/v1/pair", None, Some(json!({"code": "0000000000", "name": "x"}))).await;
    assert_eq!(wrong.0, StatusCode::FORBIDDEN);
    let (status, body) =
        send(&app, Method::POST, "/api/v1/pair", None, Some(json!({"code": code, "name": "Browser"}))).await;
    assert_eq!(status, StatusCode::OK);
    let token = body["token"].as_str().unwrap();
    let again = send(&app, Method::POST, "/api/v1/pair", None, Some(json!({"code": code, "name": "x"}))).await;
    assert_eq!(again.0, StatusCode::FORBIDDEN, "a code is used once");
    assert_eq!(send(&app, Method::GET, "/api/v1/books", Some(token), None).await, (StatusCode::OK, json!([])));
    // The description is open to anyone on the device; the web app around the API too.
    assert_eq!(send(&app, Method::GET, "/api/v1/openapi.json", None, None).await.0, StatusCode::OK);
    let devices = send(&app, Method::GET, "/api/v1/devices", Some(token), None).await.1;
    assert_eq!(devices[0]["name"], "Browser");
    let device = devices[0]["id"].as_str().unwrap();
    assert_eq!(
        send(&app, Method::DELETE, &format!("/api/v1/devices/{device}"), Some(token), None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        send(&app, Method::GET, "/api/v1/books", Some(token), None).await.0,
        StatusCode::UNAUTHORIZED,
        "revoked"
    );
}

#[tokio::test]
async fn a_freelancer_books_and_reports_through_the_api() {
    let (_scratch, api, app) = setup();
    let token = paired(&api, &app).await;
    let token = Some(token.as_str());
    let mut changes = api.changes.subscribe();
    let (status, books) = send(
        &app,
        Method::POST,
        "/api/v1/books",
        token,
        Some(json!({
            "name": "Ada", "from": "2026-01-01", "accounting": "de-cash-basis", "vat_scheme": "de-regular",
            "vat_basis": "cash", "vat_period": "quarter", "chart": "de-skr03",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{books}");
    let id = books["id"].as_str().unwrap().to_owned();
    assert_eq!(changes.recv().await.unwrap().books, id, "a change is announced");
    let base = format!("/api/v1/books/{id}");

    let sale = json!({"date": "2026-02-10", "text": "Rechnung 1", "account": "8400", "gross": "1190.00", "tax": "de-out-19", "payment": "bank"});
    let (status, written) = send(&app, Method::POST, &format!("{base}/bookings/simple"), token, Some(sale)).await;
    assert_eq!(status, StatusCode::OK, "{written}");
    let lines = json!({"date": "2026-02-11", "text": "Papier", "lines": [
        {"account": "4930", "amount": "10.00", "tax": "de-in-19"}, {"account": "1576", "amount": "1.90", "tax": "de-in-19"},
        {"account": "1200", "amount": "-11.90"}]});
    assert_eq!(send(&app, Method::POST, &format!("{base}/bookings/lines"), token, Some(lines)).await.0, StatusCode::OK);
    let unbalanced = json!({"date": "2026-02-11", "text": "x", "lines": [{"account": "4930", "amount": "1.00"}]});
    let (status, problem) = send(&app, Method::POST, &format!("{base}/bookings/lines"), token, Some(unbalanced)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(problem["error"].as_str().is_some());

    let journal = send(&app, Method::GET, &format!("{base}/journal?year=2026&lang=en"), token, None).await.1;
    assert_eq!(journal.as_array().unwrap().len(), 2);
    assert_eq!(journal[0]["amount"], json!({"value": "1190.00", "currency": "EUR"}));
    assert_eq!(journal[0]["postings"][0]["number"], "8400");
    assert_eq!(journal[1]["postings"][0]["name"], "Office supplies");

    let overview = send(&app, Method::GET, &format!("{base}/overview?year=2026"), token, None).await.1;
    assert_eq!(overview["bank"]["value"], "1178.10");
    assert_eq!(
        (overview["income"]["value"].as_str(), overview["expenses"]["value"].as_str()),
        (Some("1000.00"), Some("10.00"))
    );

    let reports = send(&app, Method::GET, &format!("{base}/reports?year=2026"), token, None).await.1;
    assert_eq!(reports[0]["due"], "2026-04-10");
    let figures =
        send(&app, Method::GET, &format!("{base}/reports/de-ustva?from=2026-01-01&to=2026-03-31"), token, None).await.1;
    let field = |name: &str| {
        figures["fields"].as_array().unwrap().iter().find(|f| f["field"] == name).map(|f| f["amount"]["value"].clone())
    };
    assert_eq!((field("81"), field("66")), (Some(json!("1000.00")), Some(json!("1.90"))));
    assert!(figures["law"].as_str().unwrap().starts_with("spezado-law-de/"));

    let event = journal[1]["event"].as_str().unwrap();
    let reversal = json!({"event": event, "date": "2026-02-12", "text": "falsch"});
    assert_eq!(send(&app, Method::POST, &format!("{base}/reversals"), token, Some(reversal)).await.0, StatusCode::OK);
    assert_eq!(send(&app, Method::GET, &format!("{base}/review"), token, None).await.1, json!([]));
    assert_eq!(
        send(&app, Method::POST, &format!("{base}/closings"), token, Some(json!({"to": "2026-03-31"}))).await.0,
        StatusCode::OK
    );
    let late = json!({"date": "2026-03-01", "text": "spät", "account": "4930", "gross": "1.00", "payment": "cash"});
    assert_eq!(
        send(&app, Method::POST, &format!("{base}/bookings/simple"), token, Some(late)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let accounts = send(&app, Method::GET, &format!("{base}/accounts"), token, None).await.1;
    assert!(accounts.as_array().unwrap().iter().any(|a| a["number"] == "4920" && a["name"] == "Telefon"));
    let codes = send(&app, Method::GET, &format!("{base}/tax-codes"), token, None).await.1;
    assert_eq!(codes[0]["code"], "de-in-19");
    assert_eq!(
        send(&app, Method::GET, "/api/v1/books/nonsense/overview", token, None).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[test]
fn the_description_in_the_repository_is_current() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../api/openapi.json");
    let stored = fs::read_to_string(&path).unwrap_or_default();
    assert!(
        stored.trim_end() == utoipa_openapi().trim_end(),
        "api/openapi.json is out of date – write it again with `cargo run -- openapi > api/openapi.json`"
    );
}

#[tokio::test]
async fn the_web_app_is_served_around_the_api() {
    let (_scratch, _api, app) = setup();
    for path in ["/", "/journal", "/anything/else"] {
        let response = app.clone().oneshot(Request::builder().uri(path).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let html = response.headers()[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/html");
        assert!(html, "{path}: the app, which routes by the address");
        if crate::WebApp::get("index.html").is_some() {
            let policy = response.headers()[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
            assert!(policy.contains("default-src 'self'") && policy.contains("frame-ancestors 'none'"));
        }
    }
    let api =
        app.clone().oneshot(Request::builder().uri("/api/v1/nothing").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(api.status(), StatusCode::NOT_FOUND, "the API does not fall back to the app");
}

/// Sends bytes; returns the status and the body as JSON.
async fn upload(app: &Router, uri: &str, token: &str, bytes: &[u8]) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(bytes.to_vec()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

/// Gets bytes; returns the status, the content type and the bytes.
async fn download(app: &Router, uri: &str, token: &str) -> (StatusCode, String, Vec<u8>) {
    let request = Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let kind = response.headers().get(header::CONTENT_TYPE).map(|v| v.to_str().unwrap().to_owned()).unwrap_or_default();
    (status, kind, response.into_body().collect().await.unwrap().to_bytes().to_vec())
}

const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../spezado-app/tests/data/");

#[tokio::test]
async fn documents_and_the_bank_through_the_api() {
    let (_scratch, api, app) = setup();
    let token = paired(&api, &app).await;
    let (_, books) = send(
        &app,
        Method::POST,
        "/api/v1/books",
        Some(&token),
        Some(json!({
            "name": "Ada", "from": "2026-01-01", "accounting": "de-cash-basis", "vat_scheme": "de-regular",
            "vat_basis": "cash", "vat_period": "quarter", "chart": "de-skr03",
        })),
    )
    .await;
    let base = format!("/api/v1/books/{}", books["id"].as_str().unwrap());
    let get = |uri: String| {
        let (app, token) = (app.clone(), token.clone());
        async move { send(&app, Method::GET, &uri, Some(&token), None).await }
    };
    let post = |uri: String, body: Value| {
        let (app, token) = (app.clone(), token.clone());
        async move { send(&app, Method::POST, &uri, Some(&token), Some(body)).await }
    };

    // The bank: October in camt.053, three entries; the telephone by hand.
    let october = fs::read(format!("{DATA}statement-001.08.xml")).unwrap();
    let (status, imported) = upload(&app, &format!("{base}/bank/statements?name=oktober.xml"), &token, &october).await;
    assert_eq!((status, imported["entries"].as_u64()), (StatusCode::OK, Some(3)), "{imported}");
    let entries = get(format!("{base}/bank/entries")).await.1;
    let telephone = entries.as_array().unwrap().iter().find(|e| e["amount"]["value"] == "-59.50").unwrap().clone();
    let book = json!({"document": telephone["document"], "entry": telephone["entry"], "account": "4920", "tax": "de-in-19", "text": "Telefon"});
    assert_eq!(post(format!("{base}/bank/entries/book"), book).await.0, StatusCode::OK);
    // January in MT940: the telephone again, proposed as before and accepted.
    let january = fs::read(format!("{DATA}statement.mt940")).unwrap();
    upload(&app, &format!("{base}/bank/statements"), &token, &january).await;
    let entries = get(format!("{base}/bank/entries")).await.1;
    let again = entries.as_array().unwrap().iter().find(|e| e["amount"]["value"] == "-59.50").unwrap().clone();
    assert_eq!(again["suggestion"]["source"], "history");
    assert_eq!(again["suggestion"]["booking"]["postings"][0]["number"], "4920");
    let reference = json!({"document": again["document"], "entry": again["entry"]});
    assert_eq!(post(format!("{base}/bank/entries/accept"), reference).await.0, StatusCode::OK);
    assert_eq!(post(format!("{base}/bank/accept-all"), json!(null)).await, (StatusCode::OK, json!({"count": 0})));
    assert_eq!(post(format!("{base}/bank/fetch"), json!(null)).await.0, StatusCode::NOT_IMPLEMENTED);

    // An e-invoice: read, proposed, recorded with its booking dated when paid; its original comes back unchanged.
    let invoice = fs::read(format!("{DATA}01.01a-INVOICE_ubl.xml")).unwrap();
    let (status, read) =
        upload(&app, &format!("{base}/documents?name=invoice.xml&side=purchase"), &token, &invoice).await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read["document"]["number"], "123456XX");
    assert!(read["proposal"]["postings"].as_array().is_some_and(|p| p.len() >= 3));
    let pending = read["pending"].as_str().unwrap();
    let record = json!({"book": true, "date": "2026-10-01", "text": "Rechnung 123456XX"});
    assert_eq!(post(format!("{base}/documents/pending/{pending}"), record.clone()).await.0, StatusCode::OK);
    assert_eq!(post(format!("{base}/documents/pending/{pending}"), record).await.0, StatusCode::NOT_FOUND, "once");
    let documents = get(format!("{base}/documents")).await.1;
    let recorded = documents.as_array().unwrap().iter().find(|d| d["number"] == "123456XX").unwrap();
    assert_eq!(recorded["booked"], true);
    let (status, kind, bytes) =
        download(&app, &format!("{base}/documents/{}/original", recorded["id"].as_str().unwrap()), &token).await;
    assert_eq!((status, kind.as_str(), bytes == invoice), (StatusCode::OK, "application/xml", true));

    // A photo: filed alone, then a booking rests on it.
    let (_, photo) =
        upload(&app, &format!("{base}/documents?name=beleg.jpg"), &token, b"\xff\xd8\xff\xe0 a photo").await;
    assert!(photo["proposal"].is_null());
    post(format!("{base}/documents/pending/{}", photo["pending"].as_str().unwrap()), json!({"book": false})).await;
    let id = photo["document"]["id"].as_str().unwrap();
    let paper = json!({"date": "2026-10-20", "text": "Papier", "account": "4930", "gross": "11.90", "tax": "de-in-19", "payment": "cash", "documents": [id]});
    assert_eq!(post(format!("{base}/bookings/simple"), paper).await.0, StatusCode::OK);
    let journal = get(format!("{base}/journal?year=2026")).await.1;
    assert!(journal.as_array().unwrap().iter().any(|e| e["documents"] == json!([id])));

    // The data transfer of the GoBD as a ZIP file.
    let (status, kind, zip) = download(&app, &format!("{base}/gobd?from=2026-01-01&to=2026-12-31"), &token).await;
    assert_eq!((status, kind.as_str()), (StatusCode::OK, "application/zip"));
    assert!(zip.starts_with(b"PK") && zip.windows(9).any(|w| w == b"index.xml"));
}

#[tokio::test]
async fn opening_balances_through_the_api() {
    let (_scratch, api, app) = setup();
    let token = paired(&api, &app).await;
    let (_, books) = send(
        &app,
        Method::POST,
        "/api/v1/books",
        Some(&token),
        Some(json!({
            "name": "Ada", "from": "2026-01-01", "accounting": "de-cash-basis", "vat_scheme": "de-regular",
            "vat_basis": "cash", "vat_period": "quarter", "chart": "de-skr03",
        })),
    )
    .await;
    let base = format!("/api/v1/books/{}", books["id"].as_str().unwrap());
    let (status, view) = send(&app, Method::GET, &format!("{base}/opening"), Some(&token), None).await;
    assert_eq!(
        (status, &view["start"], view["balances"].as_array().unwrap().len()),
        (StatusCode::OK, &json!("2026-01-01"), 0)
    );
    let opening = json!({"lines": [{"account": "1200", "amount": "3160.50"}, {"account": "1000", "amount": "0"}]});
    let (status, written) = send(&app, Method::POST, &format!("{base}/opening"), Some(&token), Some(opening)).await;
    assert_eq!(status, StatusCode::OK, "{written}");
    let (_, view) = send(&app, Method::GET, &format!("{base}/opening?lang=en"), Some(&token), None).await;
    assert_eq!(
        view["balances"],
        json!([{"account": view["balances"][0]["account"], "number": "1200", "name": "Bank", "amount": {"value": "3160.50", "currency": "EUR"}}])
    );
    let empty = json!({"lines": []});
    assert_eq!(
        send(&app, Method::POST, &format!("{base}/opening"), Some(&token), Some(empty)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn synchronisation_is_reported_and_refused_without_a_relay() {
    let (_scratch, api, app) = setup();
    let token = paired(&api, &app).await;
    let (_, books) = send(
        &app,
        Method::POST,
        "/api/v1/books",
        Some(&token),
        Some(json!({
            "name": "Ada", "from": "2026-01-01", "accounting": "de-cash-basis", "vat_scheme": "de-regular",
            "vat_basis": "cash", "vat_period": "quarter", "chart": "de-skr03",
        })),
    )
    .await;
    let base = format!("/api/v1/books/{}/sync", books["id"].as_str().unwrap());
    let (status, view) = send(&app, Method::GET, &base, Some(&token), None).await;
    assert_eq!((status, view), (StatusCode::OK, json!({"relay": null})));
    assert_eq!(send(&app, Method::POST, &base, Some(&token), None).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let join = json!({"invitation": "not an invitation"});
    assert_eq!(
        send(&app, Method::POST, "/api/v1/sync/join", Some(&token), Some(join)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn deadlines_change_through_the_api() {
    let (_scratch, api, app) = setup();
    let token = paired(&api, &app).await;
    let (_, books) = send(
        &app,
        Method::POST,
        "/api/v1/books",
        Some(&token),
        Some(json!({
            "name": "Ada", "from": "2026-01-01", "accounting": "de-cash-basis", "vat_scheme": "de-regular",
            "vat_basis": "cash", "vat_period": "quarter", "chart": "de-skr03",
        })),
    )
    .await;
    let base = format!("/api/v1/books/{}/deadlines", books["id"].as_str().unwrap());
    let (_, before) = send(&app, Method::GET, &base, Some(&token), None).await;
    assert_eq!(before, json!({"from": null, "region": null, "vat_extended": false}));
    let change = json!({"from": "2026-01-01", "region": "DE-SN", "vat_extended": true});
    let (status, changed) = send(&app, Method::PUT, &base, Some(&token), Some(change.clone())).await;
    assert_eq!((status, changed), (StatusCode::OK, change));
    let (_, after) = send(&app, Method::GET, &base, Some(&token), None).await;
    assert_eq!((&after["region"], &after["vat_extended"]), (&json!("DE-SN"), &json!(true)));
    let wrong = json!({"region": "Sachsen", "vat_extended": false});
    assert_eq!(send(&app, Method::PUT, &base, Some(&token), Some(wrong)).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn invoices_through_the_api() {
    let (_scratch, api, app) = setup();
    let token = paired(&api, &app).await;
    let (_, books) = send(
        &app,
        Method::POST,
        "/api/v1/books",
        Some(&token),
        Some(json!({
            "name": "Ada", "from": "2026-01-01", "accounting": "de-cash-basis", "vat_scheme": "de-regular",
            "vat_basis": "cash", "vat_period": "quarter", "chart": "de-skr03",
        })),
    )
    .await;
    let base = format!("/api/v1/books/{}", books["id"].as_str().unwrap());
    assert_eq!(send(&app, Method::GET, &format!("{base}/invoicing/profile"), Some(&token), None).await.1, json!(null));
    let profile = json!({
        "name": "Ada Beispiel", "street": "Musterweg 1", "postcode": "10115", "city": "Berlin", "country": "DE",
        "vat_id": "DE123456789", "tax_number": null, "contact": "Ada Beispiel", "phone": "+49 30 1234567",
        "email": "ada@example.org", "iban": "DE02120300000000202051", "payment_days": 14, "prefix": "RE-",
    });
    assert_eq!(
        send(&app, Method::PUT, &format!("{base}/invoicing/profile"), Some(&token), Some(profile)).await.0,
        StatusCode::OK
    );
    let customer = json!({
        "name": "Kunde A GmbH", "street": "Hauptstraße 5", "postcode": "80331", "city": "München", "country": "DE",
        "email": "rechnung@kunde-a.example", "vat_id": null, "reference": null,
    });
    let (_, saved) = send(&app, Method::POST, &format!("{base}/customers"), Some(&token), Some(customer)).await;
    let (_, next) =
        send(&app, Method::GET, &format!("{base}/invoices/next-number?year=2026"), Some(&token), None).await;
    assert_eq!(next["number"], "RE-2026-001");
    let invoice = json!({
        "customer": saved["id"], "date": "2026-09-30", "delivery": null, "note": null,
        "lines": [{"name": "Honorar", "quantity": "20", "unit": "HUR", "price": "100.00", "tax": "de-out-19"}],
    });
    let (status, issued) = send(&app, Method::POST, &format!("{base}/invoices"), Some(&token), Some(invoice)).await;
    assert_eq!(status, StatusCode::OK, "{issued}");
    assert_eq!((issued["number"].as_str(), issued["due"]["value"].as_str()), (Some("RE-2026-001"), Some("2380.00")));
    let (status, _, xml) =
        download(&app, &format!("{base}/documents/{}/original", issued["document"].as_str().unwrap()), &token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(String::from_utf8(xml).unwrap().contains("RE-2026-001"));
    // As ZUGFeRD: a PDF.
    let invoice = json!({
        "customer": saved["id"], "date": "2026-10-01", "format": "zugferd",
        "lines": [{"name": "Honorar", "quantity": "1", "unit": "DAY", "price": "800.00", "tax": "de-out-19"}],
    });
    let (status, issued) = send(&app, Method::POST, &format!("{base}/invoices"), Some(&token), Some(invoice)).await;
    assert_eq!((status, issued["name"].as_str()), (StatusCode::OK, Some("RE-2026-002.pdf")), "{issued}");
    let (status, _, pdf) =
        download(&app, &format!("{base}/documents/{}/original", issued["document"].as_str().unwrap()), &token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn the_certificate_is_kept_for_its_host() {
    let scratch = Scratch::new("tls");
    let workspace = Workspace::create(&scratch.0, "server".parse().unwrap()).unwrap();
    let (_, first) = crate::tls::configuration(&workspace, "spezado.test").unwrap();
    let (_, again) = crate::tls::configuration(&workspace, "spezado.test").unwrap();
    assert!(first.starts_with("sha256:") && first.len() == 7 + 64);
    assert_eq!(first, again, "the same certificate after a restart");
    let (_, other) = crate::tls::configuration(&workspace, "other.test").unwrap();
    assert_ne!(first, other, "a new one for another host name");
}
