// SPDX-License-Identifier: AGPL-3.0-or-later
//! The routes of the payroll (librotenado spec part 0013): employments, payslips, the Beitragsnachweis.

use std::collections::BTreeMap;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use librotenado::payroll::{GermanEmployment, GermanInsurance, GermanTax, PayItem, PayItemKind, Payslip};
use librotenado::{BooksId, Currency, Date, Decimal, Money, PartyId, Person};
use serde::Deserialize;
use spezado_app::{NewEmployment, Workspace, new_id, today};
use spezado_core::{Command, format_amount, parse_amount};
use utoipa::IntoParams;

use crate::{Api, ApiError, dto};

/// The routes of the payroll.
pub fn routes() -> Router<Api> {
    Router::new()
        .route("/books/{books}/employments", get(employments).post(add_employment))
        .route("/books/{books}/payslips", get(payslips).post(payslip))
        .route("/books/{books}/partners", get(partners))
        .route("/books/{books}/partners/{partner}/capital-accounts", post(partner_accounts))
        .route("/books/{books}/contributions", get(contributions))
        .route("/books/{books}/employments/{employment}/certificate", get(certificate))
}

fn books_id(text: &str) -> Result<BooksId, ApiError> {
    text.parse().map_err(|_| ApiError::bad(format!("not the identity of books: {text:?}")))
}

fn owner(workspace: &Workspace, books: &BooksId) -> Result<PartyId, ApiError> {
    let owners = workspace.get(books)?.events().owners();
    owners.first().map(|o| (*o).clone()).ok_or_else(|| ApiError::bad("the books have no owner"))
}

fn date(text: &str) -> Result<Date, ApiError> {
    text.parse().map_err(|_| ApiError::bad(format!("not a date YYYY-MM-DD: {text:?}")))
}

fn decimal(text: &Option<String>) -> Result<Option<Decimal>, ApiError> {
    text.as_deref()
        .filter(|t| !t.is_empty())
        .map(|t| t.parse().map_err(|_| ApiError::bad(format!("not a number: {t:?}"))))
        .transpose()
}

fn eur() -> Currency {
    "EUR".parse().expect("EUR")
}

fn kind_name(kind: PayItemKind) -> &'static str {
    match kind {
        PayItemKind::Wage => "wage",
        PayItemKind::OneOff => "one_off",
        PayItemKind::Compensation => "compensation",
        PayItemKind::InKind => "in_kind",
        PayItemKind::TaxFree => "tax_free",
        PayItemKind::Deduction => "deduction",
        PayItemKind::Pension => "pension",
    }
}

fn view(payslip: &Payslip, recorded: bool) -> dto::PayslipView {
    dto::PayslipView {
        id: payslip.id.to_string(),
        employment: payslip.employment.to_string(),
        from: payslip.from.to_string(),
        to: payslip.to.to_string(),
        paid: payslip.paid.to_string(),
        items: payslip
            .items
            .iter()
            .map(|i| dto::PayItem {
                kind: kind_name(i.kind).into(),
                name: i.name.clone(),
                amount: format_amount(&i.amount),
            })
            .collect(),
        figures: payslip.figures.iter().map(|(n, m)| (n.clone(), format_amount(m))).collect(),
        recorded,
        replaces: payslip.replaces.as_ref().map(ToString::to_string),
    }
}

/// The employments of the owner as described today.
#[utoipa::path(get, path = "/api/v1/books/{books}/employments", params(("books" = String, Path)),
    responses((status = 200, body = Vec<dto::EmploymentView>)))]
pub(crate) async fn employments(
    State(api): State<Api>,
    Path(books): Path<String>,
) -> Result<Json<Vec<dto::EmploymentView>>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let (_, persons) = workspace.get(&books)?.events().parties();
        let names: BTreeMap<PartyId, String> = persons.iter().map(|(id, p)| ((*id).clone(), p.name.clone())).collect();
        Ok(Json(
            workspace
                .employments(&books, &owner, today())?
                .into_iter()
                .map(|e| {
                    let de = e.de.as_ref();
                    dto::EmploymentView {
                        id: e.id.to_string(),
                        employee: e.employee.to_string(),
                        name: names.get(&e.employee).cloned().unwrap_or_default(),
                        start: e.start.to_string(),
                        end: e.end.map(|d| d.to_string()),
                        number: e.number.clone(),
                        class: de.and_then(|de| de.tax.class),
                        insurer: de.map(|de| de.insurance.insurer.clone()),
                        contribution_group: de.map(|de| de.insurance.contribution_group.clone()),
                        person_group: de.map(|de| de.insurance.person_group.clone()),
                    }
                })
                .collect(),
        ))
    })
    .await
}

/// Adds an employment under German law; the employee is described as a person by name. Returns its identity.
#[utoipa::path(post, path = "/api/v1/books/{books}/employments", params(("books" = String, Path)),
    request_body = dto::NewEmployment, responses((status = 200, body = String), (status = 422, body = dto::Problem)))]
pub(crate) async fn add_employment(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(request): Json<dto::NewEmployment>,
) -> Result<Json<String>, ApiError> {
    let id = books.clone();
    let added = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let owner = owner(workspace, &books)?;
            let name = request.name.trim().to_owned();
            if name.is_empty() {
                return Err(ApiError::bad("an employee needs a name"));
            }
            let de = GermanEmployment {
                tax: GermanTax {
                    class: request.class,
                    factor: None,
                    children: decimal(&request.children)?,
                    church: request.church.clone().filter(|c| !c.is_empty()),
                    allowance: None,
                    addition: None,
                    flat: request.flat,
                    short_term: request.short_term,
                    pension_start: match (request.pension_start_year, &request.pension_start_monthly) {
                        (Some(year), Some(monthly)) => Some(librotenado::payroll::PensionStart {
                            year,
                            monthly: parse_amount(monthly, &eur()).map_err(|e| ApiError::bad(e.to_string()))?,
                        }),
                        _ => None,
                    },
                    active_pension: request.active_pension,
                },
                insurance: GermanInsurance {
                    insurer: request.insurer.clone(),
                    contribution_group: request.contribution_group.clone().unwrap_or_else(|| "1111".into()),
                    person_group: request.person_group.clone().unwrap_or_else(|| "101".into()),
                    additional_rate: decimal(&request.additional_rate)?,
                    private: None,
                    childless: request.childless,
                    children: request.care_children,
                    saxony: request.saxony,
                    u1: decimal(&request.u1)?,
                    u2: decimal(&request.u2)?,
                    household: request.household,
                },
            };
            let new = NewEmployment {
                owner,
                employee: new_id(),
                start: date(&request.start)?,
                end: request.end.as_deref().map(date).transpose()?,
                number: request.number.clone(),
                born: request.born.as_deref().map(date).transpose()?,
                de,
            };
            let person: Person = serde_json::from_value(serde_json::json!({"id": new.employee.as_str(), "name": name}))
                .map_err(|e| ApiError::bad(e.to_string()))?;
            workspace.execute(&books, Command::DescribePerson(person))?;
            Ok(workspace.add_employment(&books, new)?.to_string())
        })
        .await?;
    api.changed(&id);
    Ok(Json(added))
}

/// Computes the payslip of an employment for a month; with `record` records it and books it.
#[utoipa::path(post, path = "/api/v1/books/{books}/payslips", params(("books" = String, Path)),
    request_body = dto::PayslipRequest, responses((status = 200, body = dto::PayslipView), (status = 422, body = dto::Problem)))]
pub(crate) async fn payslip(
    State(api): State<Api>,
    Path(books): Path<String>,
    Json(request): Json<dto::PayslipRequest>,
) -> Result<Json<dto::PayslipView>, ApiError> {
    let id = books.clone();
    let record = request.record;
    let result = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let correction = request.corrects.is_some();
            let employment = match request.employment.parse() {
                Ok(id) => id,
                Err(_) if correction => new_id(),
                Err(_) => return Err(ApiError::bad("not the identity of an employment")),
            };
            let (year, month) = match request
                .month
                .split_once('-')
                .and_then(|(y, m)| Some((y.parse::<u16>().ok()?, m.parse::<u8>().ok()?)))
            {
                Some(month) => month,
                None if correction => (0, 0),
                None => return Err(ApiError::bad("a month is written YYYY-MM")),
            };
            let mut items = Vec::new();
            for item in &request.items {
                let kind = match item.kind.as_str() {
                    "wage" => PayItemKind::Wage,
                    "one_off" => PayItemKind::OneOff,
                    "compensation" => PayItemKind::Compensation,
                    "in_kind" => PayItemKind::InKind,
                    "tax_free" => PayItemKind::TaxFree,
                    "deduction" => PayItemKind::Deduction,
                    "pension" => PayItemKind::Pension,
                    other => return Err(ApiError::bad(format!("not a kind of item: {other:?}"))),
                };
                let amount: Money = parse_amount(&item.amount, &eur()).map_err(|e| ApiError::bad(e.to_string()))?;
                items.push(PayItem { kind, name: item.name.clone(), amount });
            }
            let payslip = match &request.corrects {
                Some(replaced) => {
                    let replaced = replaced.parse().map_err(|_| ApiError::bad("not the identity of a payslip"))?;
                    let paid = request.paid.as_deref().map(date).transpose()?.unwrap_or_else(today);
                    workspace.correction(&books, &replaced, items, paid)?
                }
                None => workspace.payslip(&books, &employment, year, month, items)?,
            };
            let view = view(&payslip, record);
            if record {
                workspace.record_payslip(&books, payslip)?;
            }
            Ok(view)
        })
        .await?;
    if record {
        api.changed(&id);
    }
    Ok(Json(result))
}

/// The period and insurer of a Beitragsnachweis.
#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct ContributionQuery {
    /// The company number of the insurer.
    insurer: String,
    /// The first day, `YYYY-MM-DD`.
    from: String,
    /// The last day, `YYYY-MM-DD`.
    to: String,
}

/// The Beitragsnachweis of the owner to one insurer: the contributions by the groups of the record, in euros.
#[utoipa::path(get, path = "/api/v1/books/{books}/contributions", params(("books" = String, Path), ContributionQuery),
    responses((status = 200, body = BTreeMap<String, String>)))]
pub(crate) async fn contributions(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<ContributionQuery>,
) -> Result<Json<BTreeMap<String, String>>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let fields =
            workspace.contribution_statement(&books, &owner, &query.insurer, date(&query.from)?, date(&query.to)?)?;
        Ok(Json(
            fields
                .into_iter()
                .map(|(field, units)| (field, format_amount(&Money::new(units, eur()).expect("an amount"))))
                .collect(),
        ))
    })
    .await
}

/// The calendar year of a certificate.
#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct CertificateQuery {
    /// The calendar year.
    year: u16,
}

/// The Lohnsteuerbescheinigung of an employment for a year: its numbered lines in euros (names `de-lstb/<line>`);
/// empty for wages taxed at a flat rate or a year without payslips.
#[utoipa::path(get, path = "/api/v1/books/{books}/employments/{employment}/certificate",
    params(("books" = String, Path), ("employment" = String, Path), CertificateQuery),
    responses((status = 200, body = BTreeMap<String, String>)))]
pub(crate) async fn certificate(
    State(api): State<Api>,
    Path((books, employment)): Path<(String, String)>,
    Query(query): Query<CertificateQuery>,
) -> Result<Json<BTreeMap<String, String>>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let employment = employment.parse().map_err(|_| ApiError::bad("not the identity of an employment"))?;
        let lines = workspace.wage_tax_certificate(&books, &employment, query.year)?.unwrap_or_default();
        Ok(Json(
            lines
                .into_iter()
                .map(|(line, units)| (line, format_amount(&Money::new(units, eur()).expect("an amount"))))
                .collect(),
        ))
    })
    .await
}

/// The calendar year of the payslips.
#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct YearQuery {
    /// The calendar year; default this one.
    year: Option<u16>,
}

/// The payslips that apply for periods ending in a calendar year – corrections in place of what they replace.
#[utoipa::path(get, path = "/api/v1/books/{books}/payslips", params(("books" = String, Path), YearQuery),
    responses((status = 200, body = Vec<dto::PayslipView>)))]
pub(crate) async fn payslips(
    State(api): State<Api>,
    Path(books): Path<String>,
    Query(query): Query<YearQuery>,
) -> Result<Json<Vec<dto::PayslipView>>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let year = query.year.unwrap_or_else(|| today().year());
        let (from, to) = (Date::new(year, 1, 1).expect("a date"), Date::new(year, 12, 31).expect("a date"));
        Ok(Json(workspace.payslips(&books, &owner, from, to)?.iter().map(|p| view(p, true)).collect()))
    })
    .await
}

/// How a partner is liable: `unlimited` or `limited`.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub(crate) struct PartnerAccounts {
    /// `unlimited` (Vollhafter) or `limited` (Teilhafter).
    liability: String,
}

/// Opens the capital accounts of a partner for the Kapitalkontenentwicklung of the E-Bilanz; returns their identities.
#[utoipa::path(post, path = "/api/v1/books/{books}/partners/{partner}/capital-accounts",
    params(("books" = String, Path), ("partner" = String, Path)),
    request_body = PartnerAccounts, responses((status = 200, body = Vec<String>), (status = 422, body = dto::Problem)))]
pub(crate) async fn partner_accounts(
    State(api): State<Api>,
    Path((books, partner)): Path<(String, String)>,
    Json(request): Json<PartnerAccounts>,
) -> Result<Json<Vec<String>>, ApiError> {
    let id = books.clone();
    let opened = api
        .run(move |workspace| {
            let books = books_id(&books)?;
            let partner: PartyId = partner.parse().map_err(|_| ApiError::bad("not the identity of a party"))?;
            let liability = match request.liability.as_str() {
                "unlimited" => librotenado::Liability::Unlimited,
                "limited" => librotenado::Liability::Limited,
                other => return Err(ApiError::bad(format!("not a liability: {other:?}"))),
            };
            Ok(workspace
                .open_partner_accounts(&books, &partner, liability)?
                .into_iter()
                .map(|(account, _)| account.to_string())
                .collect())
        })
        .await?;
    api.changed(&id);
    Ok(Json(opened))
}

/// A partner of the owner: a person with the role `partner` in it.
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct Partner {
    /// The person.
    id: String,
    /// Their name.
    name: String,
    /// Whether capital accounts are opened for them.
    accounts: bool,
}

/// The partners of the owner, a partnership, and whether their capital accounts are opened.
#[utoipa::path(get, path = "/api/v1/books/{books}/partners", params(("books" = String, Path)),
    responses((status = 200, body = Vec<Partner>)))]
pub(crate) async fn partners(
    State(api): State<Api>,
    Path(books): Path<String>,
) -> Result<Json<Vec<Partner>>, ApiError> {
    api.run(move |workspace| {
        let books = books_id(&books)?;
        let owner = owner(workspace, &books)?;
        let events = workspace.get(&books)?.events();
        let (_, persons) = events.parties();
        let opened: Vec<String> = events
            .ordered()
            .into_iter()
            .filter_map(|(_, e)| match &e.payload {
                librotenado::Payload::AccountOpened { account } => account.attributes.get("partner").cloned(),
                _ => None,
            })
            .collect();
        Ok(Json(
            persons
                .values()
                .filter(|p| p.roles.iter().any(|r| r.organisation == owner && r.kind == librotenado::RoleKind::Partner))
                .map(|p| Partner {
                    id: p.id.to_string(),
                    name: p.name.clone(),
                    accounts: opened.contains(&p.id.to_string()),
                })
                .collect(),
        ))
    })
    .await
}
