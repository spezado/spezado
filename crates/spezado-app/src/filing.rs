// SPDX-License-Identifier: AGPL-3.0-or-later
//! Filings (spec part 0007 of librotenado): derived from the books, recorded, and submitted through an adapter – a
//! service of its own per country that talks to the authority, such as ELSTER in Germany.

use std::time::Duration;

use librotenado::{
    AdapterError, Country, Date, Derivation, Document, DocumentId, DocumentKind, Filer, Filing, FilingId, FilingKind,
    FilingState, Payload, ReportContent, Submission, SubmissionState, SubmissionStatus, from_root, to_root,
};
use serde::{Deserialize, Serialize};
use spezado_core::Command;
use spezado_law::{Facts, ReportBasis};

use crate::{AppError, BooksId, PartyId, Workspace, new_id, sync::KEYCHAIN, today};

/// The setting of a country's adapter, in the workspace: `adapter:DE`.
fn setting_key(country: Country) -> String {
    format!("adapter:{country}")
}

/// The largest answer read from an adapter: receipts are protocols and notices, not archives.
const MAX_ANSWER: u64 = 64 * 1024 * 1024;

/// Where a country's adapter is, and how this device authenticates itself to it.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AdapterSetting {
    url: String,
    /// The token, unless it is in the key store.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    token: String,
    /// `keychain` when the token is in the operating system's key store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    token_in: Option<String>,
}

/// A filing of a set of books and where it stands.
#[derive(Debug, Clone)]
pub struct FilingItem {
    /// The filing, as prepared.
    pub filing: Filing,
    /// Where it stands.
    pub state: FilingState,
    /// The books changed since it was prepared so that its content would differ now; it has to be prepared again.
    pub outdated: bool,
    /// The documents of what the authority returned, in the order received.
    pub documents: Vec<DocumentId>,
}

impl Workspace {
    /// Sets the adapter that submits filings in `country`: its address and the token this device authenticates
    /// itself with (a bearer token, spec part 0007). The token goes into the key store if it takes it.
    pub fn set_adapter(&mut self, country: Country, url: &str, token: &str) -> Result<(), AppError> {
        let url = url.trim().trim_end_matches('/');
        if !(url.starts_with("https://") || url.starts_with("http://127.0.0.1") || url.starts_with("http://localhost"))
        {
            return Err(AppError::Adapter("an adapter is reached over HTTPS, or over HTTP on this device".into()));
        }
        if token.is_empty() {
            return Err(AppError::Adapter("an adapter needs a token".into()));
        }
        let account = format!("adapter {country} {}", self.device);
        let stored = self.keychain
            && keyring::Entry::new(KEYCHAIN, &account)
                .and_then(|entry| entry.set_secret(token.as_bytes()).and_then(|()| entry.get_secret()))
                .is_ok_and(|back| back == token.as_bytes());
        let setting = AdapterSetting {
            url: url.to_owned(),
            token: if stored { String::new() } else { token.to_owned() },
            token_in: stored.then(|| "keychain".to_owned()),
        };
        let json = serde_json::to_string(&setting).map_err(|e| AppError::Adapter(e.to_string()))?;
        Ok(self.store.set_setting(None, &setting_key(country), &json)?)
    }

    /// The address of the adapter that submits filings in `country`, if one is set.
    pub fn adapter(&self, country: Country) -> Result<Option<String>, AppError> {
        Ok(self.adapter_setting(country)?.map(|setting| setting.url))
    }

    fn adapter_setting(&self, country: Country) -> Result<Option<AdapterSetting>, AppError> {
        let Some(json) = self.store.setting(None, &setting_key(country))? else { return Ok(None) };
        serde_json::from_str(&json).map(Some).map_err(|e| AppError::Adapter(e.to_string()))
    }

    /// Whether a report is filed on its own, as the law that applies to the owner says.
    pub fn is_filed(
        &self,
        books: &BooksId,
        owner: &PartyId,
        report: &str,
        from: Date,
        to: Date,
    ) -> Result<bool, AppError> {
        let (configuration, country, legal_form, module) = self.law_for(books, owner, from)?;
        let facts = Facts::new(&configuration, country, legal_form.as_deref());
        let reports = module.reports(&facts, from, to);
        Ok(reports.iter().any(|candidate| candidate.id == report && candidate.filing.is_some()))
    }

    /// The filings of a set of books, in the order they were prepared.
    pub fn filings(&self, books: &BooksId) -> Result<Vec<FilingItem>, AppError> {
        let events = self.get(books)?.events();
        let states = events.filings();
        let mut items = Vec::new();
        for (_, event) in events.ordered() {
            let Payload::FilingPrepared { filing } = &event.payload else { continue };
            let Some((_, state)) = states.get(&filing.id) else { continue };
            if items.iter().any(|item: &FilingItem| item.filing.id == filing.id) {
                continue;
            }
            let documents = events
                .ordered()
                .iter()
                .filter_map(|(_, event)| match &event.payload {
                    Payload::FilingSubmitted { filing: id, documents }
                    | Payload::FilingAccepted { filing: id, documents }
                    | Payload::FilingRejected { filing: id, documents }
                        if *id == filing.id =>
                    {
                        Some(documents.clone())
                    }
                    _ => None,
                })
                .flatten()
                .collect();
            let outdated = *state != FilingState::Accepted && self.content_now(books, filing)? != filing.content;
            items.push(FilingItem { filing: filing.clone(), state: *state, outdated, documents });
        }
        Ok(items)
    }

    /// Prepares the filing of a report for an owner and a period: derives its content from the books as they are and
    /// records it. A filing of the same kind and period that was submitted or accepted is corrected by it – unless
    /// the latest has the same content, which is returned as it is, as is one only prepared with the same content.
    pub fn prepare_filing(
        &mut self,
        books: &BooksId,
        owner: &PartyId,
        report: &str,
        from: Date,
        to: Date,
    ) -> Result<Filing, AppError> {
        let (kind, content, jurisdiction) = self.derive(books, owner, report, from, to)?;
        let earlier = self.filings(books)?.into_iter().filter(|item| {
            let f = &item.filing;
            f.filer == *owner && f.kind == kind && f.from == from && f.to == to
        });
        let mut latest = None;
        for item in earlier {
            match item.state {
                FilingState::Prepared if item.filing.content == [content.clone()] => return Ok(item.filing),
                FilingState::Submitted | FilingState::Accepted => latest = Some(item.filing),
                _ => {}
            }
        }
        if let Some(filed) = latest.as_ref().filter(|filed| filed.content == [content.clone()]) {
            return Ok(filed.clone());
        }
        let corrects = latest.map(|filed| filed.id);
        let filing = Filing {
            id: new_id(),
            kind,
            jurisdiction,
            filer: owner.clone(),
            from,
            to,
            basis: self.get(books)?.events().state(),
            corrects,
            content: vec![content],
        };
        self.execute(books, Command::PrepareFiling(filing.clone()))?;
        Ok(filing)
    }

    /// Submits a prepared filing through the adapter of its country and records the answer – unless `test`: a test
    /// submission the authority does not process, which changes nothing in the books.
    pub fn submit_filing(
        &mut self,
        books: &BooksId,
        filing: &FilingId,
        test: bool,
    ) -> Result<SubmissionStatus, AppError> {
        let item = self.filing_item(books, filing)?;
        if !matches!(item.state, FilingState::Prepared | FilingState::Rejected) {
            return Err(AppError::Adapter(format!("filing {filing} is submitted already")));
        }
        if item.outdated {
            return Err(AppError::Adapter(format!(
                "the books changed since filing {filing} was prepared; prepare it again"
            )));
        }
        let (organisations, persons) = self.get(books)?.events().parties();
        let filer = match (organisations.get(&item.filing.filer), persons.get(&item.filing.filer)) {
            (Some(organisation), _) => Filer::Organisation((*organisation).clone()),
            (None, Some(person)) => Filer::Person((*person).clone()),
            (None, None) => return Err(AppError::NotConfigured { owner: item.filing.filer.clone(), date: today() }),
        };
        let country = item.filing.jurisdiction;
        let submission = Submission::new(item.filing, filer, test).map_err(|e| AppError::Adapter(e.to_string()))?;
        let body = to_root(&submission).map_err(|e| AppError::Adapter(e.to_string()))?;
        let status = self.ask_adapter(country, filing, Some(body))?;
        if !test {
            self.record_status(books, &status)?;
        }
        Ok(status)
    }

    /// Asks the adapter where a submitted filing stands and records an answer that moves it on.
    pub fn refresh_filing(&mut self, books: &BooksId, filing: &FilingId) -> Result<SubmissionStatus, AppError> {
        let item = self.filing_item(books, filing)?;
        if item.state != FilingState::Submitted {
            return Err(AppError::Adapter(format!("filing {filing} is not waiting for an answer")));
        }
        let status = self.ask_adapter(item.filing.jurisdiction, filing, None)?;
        if status.state != SubmissionState::Submitted {
            self.record_status(books, &status)?;
        }
        Ok(status)
    }

    fn filing_item(&self, books: &BooksId, filing: &FilingId) -> Result<FilingItem, AppError> {
        self.filings(books)?
            .into_iter()
            .find(|item| item.filing.id == *filing)
            .ok_or_else(|| AppError::Adapter(format!("no filing {filing} in these books")))
    }

    /// The kind, content and country of a report's filing, from the books as they are.
    fn derive(
        &self,
        books: &BooksId,
        owner: &PartyId,
        report: &str,
        from: Date,
        to: Date,
    ) -> Result<(FilingKind, ReportContent, Country), AppError> {
        let (configuration, country, legal_form, module) = self.law_for(books, owner, from)?;
        let facts = Facts::new(&configuration, country, legal_form.as_deref());
        let found = module
            .reports(&facts, from, to)
            .into_iter()
            .find(|candidate| candidate.id == report)
            .ok_or_else(|| AppError::UnknownReport(report.to_owned()))?;
        let kind = found.filing.ok_or_else(|| AppError::Adapter(format!("{report} is not filed on its own")))?;
        let jurisdiction = facts.country.ok_or_else(|| AppError::NoLaw(owner.clone()))?;
        let derivation = match found.basis {
            ReportBasis::Period => Derivation::Period,
            ReportBasis::Balances => Derivation::Balances,
        };
        let taxes = module.tax_table(to).ok_or_else(|| AppError::UnknownReport(report.to_owned()))?;
        let content = self.get(books)?.events().report_content(
            &found.mapping,
            derivation,
            &taxes,
            &self.chart,
            owner,
            &configuration.currency,
            from,
            to,
        )?;
        Ok((kind, content, jurisdiction))
    }

    /// A filing's content as the books would give it now.
    fn content_now(&self, books: &BooksId, filing: &Filing) -> Result<Vec<ReportContent>, AppError> {
        filing
            .content
            .iter()
            .map(|content| {
                self.derive(books, &filing.filer, &content.report, filing.from, filing.to).map(|(_, now, _)| now)
            })
            .collect()
    }

    /// Sends a submission to the adapter of `country`, or – without a body – asks it about a filing.
    fn ask_adapter(
        &self,
        country: Country,
        filing: &FilingId,
        body: Option<Vec<u8>>,
    ) -> Result<SubmissionStatus, AppError> {
        let setting =
            self.adapter_setting(country)?.ok_or_else(|| AppError::Adapter(format!("no adapter set for {country}")))?;
        let token = match setting.token_in.as_deref() {
            Some("keychain") => keyring::Entry::new(KEYCHAIN, &format!("adapter {country} {}", self.device))
                .and_then(|entry| entry.get_secret())
                .ok()
                .and_then(|secret| String::from_utf8(secret).ok())
                .ok_or_else(|| {
                    AppError::Adapter("the adapter's token is not in the key store of this device".into())
                })?,
            _ => setting.token.clone(),
        };
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(300)))
            .build()
            .into();
        let authorization = format!("Bearer {token}");
        let unreachable = |e: ureq::Error| AppError::Adapter(format!("the adapter {}: {e}", setting.url));
        let mut response = match body {
            Some(body) => agent
                .post(format!("{}/filings", setting.url))
                .header("Authorization", &authorization)
                .header("Content-Type", "application/json")
                .send(body)
                .map_err(unreachable)?,
            None => agent
                .get(format!("{}/filings/{filing}", setting.url))
                .header("Authorization", &authorization)
                .call()
                .map_err(unreachable)?,
        };
        let code = response.status().as_u16();
        let answer = response
            .body_mut()
            .with_config()
            .limit(MAX_ANSWER)
            .read_to_vec()
            .map_err(|e| AppError::Adapter(format!("the adapter's answer: {e}")))?;
        if !(200..300).contains(&code) {
            let message = from_root::<AdapterError>(&answer)
                .map_or_else(|_| String::from_utf8_lossy(&answer).into_owned(), |error| error.message);
            return Err(AppError::Adapter(format!("the adapter refused ({code}): {message}")));
        }
        let status: SubmissionStatus =
            from_root(&answer).map_err(|e| AppError::Adapter(format!("the adapter's answer: {e}")))?;
        if status.filing != *filing {
            return Err(AppError::Adapter(format!("the adapter answered about filing {}", status.filing)));
        }
        Ok(status)
    }

    /// Records an adapter's answer: each receipt not recorded yet as a document, and the filing's new state – an
    /// answer to a filing not marked submitted yet is preceded by its submission.
    fn record_status(&mut self, books: &BooksId, status: &SubmissionStatus) -> Result<(), AppError> {
        let item = self.filing_item(books, &status.filing)?;
        let events = self.get(books)?.events();
        let known: Vec<_> = item
            .documents
            .iter()
            .filter_map(|id| {
                events.ordered().iter().find_map(|(_, event)| match &event.payload {
                    Payload::DocumentRecorded { document } if document.id == *id => Some(document.original.hash),
                    _ => None,
                })
            })
            .collect();
        let kind = if status.state == SubmissionState::Rejected {
            DocumentKind::Notice
        } else {
            DocumentKind::Acknowledgement
        };
        let day = today();
        let mut commands = Vec::new();
        let mut documents = Vec::new();
        for receipt in &status.receipts {
            let original = receipt.original();
            if known.contains(&original.hash) {
                continue;
            }
            self.store.put_original(&receipt.content.0)?;
            let document = Document {
                id: new_id(),
                kind,
                number: None,
                date: day,
                received: day,
                original,
                invoice: None,
                statement: None,
            };
            documents.push(document.id.clone());
            commands.push((books.clone(), Command::RecordDocument(document)));
        }
        let filing = status.filing.clone();
        if status.state != SubmissionState::Submitted && item.state != FilingState::Submitted {
            let state = SubmissionState::Submitted;
            commands.push((books.clone(), Command::RecordFiling { filing: filing.clone(), state, documents: vec![] }));
        }
        commands.push((books.clone(), Command::RecordFiling { filing, state: status.state, documents }));
        self.execute_all(commands)?;
        Ok(())
    }
}
