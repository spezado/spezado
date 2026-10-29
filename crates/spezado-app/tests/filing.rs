// SPDX-License-Identifier: AGPL-3.0-or-later
//! Filings: prepared from the books, submitted through an adapter – here a fake one that answers as an adapter for
//! ELSTER would – and recorded with what the authority returned.

use std::{fs, path::PathBuf};

use axum::{
    Router,
    extract::Path,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use librotenado::{
    AdapterError, BooksId, Bytes, FilingKind, FilingState, Money, PartyId, Person, Receipt, Submission,
    SubmissionState, SubmissionStatus, from_root, to_root,
};
use spezado_app::{Party, SimpleEntry, Workspace, new_id};

/// A temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("spezado-filing-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const TOKEN: &str = "a-token-of-at-least-some-length";

fn receipt(name: &str) -> Receipt {
    Receipt { media_type: "application/pdf".into(), name: name.into(), content: Bytes(name.as_bytes().to_vec()) }
}

fn answer(status: StatusCode, body: Vec<u8>) -> (StatusCode, [(&'static str, &'static str); 1], Vec<u8>) {
    (status, [("content-type", "application/json")], body)
}

fn refused(status: StatusCode, message: &str) -> (StatusCode, [(&'static str, &'static str); 1], Vec<u8>) {
    answer(status, to_root(&AdapterError { message: message.into() }).unwrap())
}

fn authorised(headers: &HeaderMap) -> bool {
    headers.get("authorization").and_then(|v| v.to_str().ok()) == Some(&format!("Bearer {TOKEN}"))
}

/// An adapter that takes every submission – a test one accepted at once, a real one with a protocol of the transfer
/// – and accepts a submitted filing when asked, returning the protocol again and a notice.
fn adapter() -> String {
    let router = Router::new()
        .route(
            "/filings",
            post(|headers: HeaderMap, body: axum::body::Bytes| async move {
                if !authorised(&headers) {
                    return refused(StatusCode::UNAUTHORIZED, "no token this adapter accepts");
                }
                let submission: Submission = match from_root(&body) {
                    Ok(submission) => submission,
                    Err(e) => return refused(StatusCode::BAD_REQUEST, &e.to_string()),
                };
                if let Err(e) = submission.check() {
                    return refused(StatusCode::UNPROCESSABLE_ENTITY, &e.to_string());
                }
                let state = if submission.test { SubmissionState::Accepted } else { SubmissionState::Submitted };
                let status = SubmissionStatus {
                    filing: submission.filing.id,
                    state,
                    receipts: vec![receipt("protocol.pdf")],
                    message: None,
                };
                answer(StatusCode::ACCEPTED, to_root(&status).unwrap())
            }),
        )
        .route(
            "/filings/{id}",
            get(|headers: HeaderMap, Path(id): Path<String>| async move {
                if !authorised(&headers) {
                    return refused(StatusCode::UNAUTHORIZED, "no token this adapter accepts");
                }
                let status = SubmissionStatus {
                    filing: id.parse().unwrap(),
                    state: SubmissionState::Accepted,
                    receipts: vec![receipt("protocol.pdf"), receipt("notice.pdf")],
                    message: None,
                };
                answer(StatusCode::OK, to_root(&status).unwrap())
            }),
        );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            axum::serve(listener, router).await.unwrap();
        });
    });
    format!("http://127.0.0.1:{}", address.port())
}

/// A freelancer on a cash basis with quarterly VAT returns, in SKR 03.
fn freelancer(scratch: &Scratch) -> (Workspace, BooksId, PartyId) {
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    workspace.use_keychain(false);
    let ada: PartyId = new_id();
    let configuration = serde_json::from_value(serde_json::json!({
        "owner": ada.as_str(), "from": "2026-01-01", "currency": "EUR", "year_start": 1, "chart": "de-skr03",
        "accounting": "de-cash-basis", "vat": {"scheme": "de-regular", "basis": "cash", "period": "quarter"},
    }))
    .unwrap();
    let person = Person { id: ada.clone(), name: "Ada".into(), tax_ids: vec![], roles: vec![], contact: None };
    let books = workspace.set_up(Party::Person(person), configuration).unwrap();
    (workspace, books, ada)
}

/// A sale at 19 % in the first quarter.
fn sell(workspace: &mut Workspace, books: &BooksId, ada: &PartyId, gross: i128) {
    let standard = workspace.standard_accounts(books, ada, "2026-02-01".parse().unwrap()).unwrap();
    let entry = SimpleEntry {
        owner: ada.clone(),
        date: "2026-02-01".parse().unwrap(),
        text: "a sale".into(),
        account: standard.sales.clone(),
        gross: Money::new(gross, "EUR".parse().unwrap()).unwrap(),
        tax: Some("de-out-19".parse().unwrap()),
        payment: standard.bank,
        documents: vec![],
    };
    let booking = workspace.simple_booking(books, &entry).unwrap();
    workspace.execute(books, spezado_core::Command::Book(booking)).unwrap();
}

#[test]
fn a_vat_return_is_prepared_submitted_and_accepted() {
    let scratch = Scratch::new("vat");
    let (mut workspace, books, ada) = freelancer(&scratch);
    sell(&mut workspace, &books, &ada, 11_900);
    let (from, to) = ("2026-01-01".parse().unwrap(), "2026-03-31".parse().unwrap());

    // Prepared from the books: the VAT return of the quarter; asked again, the same filing.
    let filing = workspace.prepare_filing(&books, &ada, "de-ustva", from, to).unwrap();
    assert_eq!(filing.kind, FilingKind::VatReturn);
    assert_eq!(filing.jurisdiction.to_string(), "DE");
    assert_eq!(filing.content[0].fields["81"].base_units(), 10_000);
    assert_eq!(workspace.prepare_filing(&books, &ada, "de-ustva", from, to).unwrap().id, filing.id);
    // The balance sheet is not filed on its own; it goes with the E-Bilanz.
    assert!(workspace.prepare_filing(&books, &ada, "de-hgb-266", from, to).is_err());

    // Without an adapter, nothing is submitted.
    let error = workspace.submit_filing(&books, &filing.id, false).unwrap_err();
    assert!(error.to_string().contains("no adapter"), "{error}");
    assert!(workspace.set_adapter("DE".parse().unwrap(), "http://example.org", TOKEN).is_err());
    let url = adapter();
    workspace.set_adapter("DE".parse().unwrap(), &url, "a-wrong-token").unwrap();
    let error = workspace.submit_filing(&books, &filing.id, false).unwrap_err();
    assert!(error.to_string().contains("(401): no token this adapter accepts"), "{error}");
    workspace.set_adapter("DE".parse().unwrap(), &format!("{url}/"), TOKEN).unwrap();
    assert_eq!(workspace.adapter("DE".parse().unwrap()).unwrap(), Some(url));

    // A test submission changes nothing in the books.
    let tested = workspace.submit_filing(&books, &filing.id, true).unwrap();
    assert_eq!(tested.state, SubmissionState::Accepted);
    assert_eq!(workspace.filings(&books).unwrap()[0].state, FilingState::Prepared);

    // Submitted: the protocol of the transfer is a document of the books.
    let status = workspace.submit_filing(&books, &filing.id, false).unwrap();
    assert_eq!(status.state, SubmissionState::Submitted);
    let items = workspace.filings(&books).unwrap();
    assert_eq!((items[0].state, items[0].documents.len(), items[0].outdated), (FilingState::Submitted, 1, false));
    assert!(workspace.submit_filing(&books, &filing.id, false).is_err());

    // A later sale in the quarter makes it outdated; the authority's acceptance is recorded, the protocol once.
    sell(&mut workspace, &books, &ada, 2_380);
    assert!(workspace.filings(&books).unwrap()[0].outdated);
    let status = workspace.refresh_filing(&books, &filing.id).unwrap();
    assert_eq!(status.state, SubmissionState::Accepted);
    let items = workspace.filings(&books).unwrap();
    assert_eq!((items[0].state, items[0].documents.len()), (FilingState::Accepted, 2));
    assert!(workspace.review(&books).unwrap().is_empty());

    // Prepared again, it corrects the accepted one.
    let correction = workspace.prepare_filing(&books, &ada, "de-ustva", from, to).unwrap();
    assert_eq!(correction.corrects, Some(filing.id));
    assert_eq!(correction.content[0].fields["81"].base_units(), 12_000);
}
