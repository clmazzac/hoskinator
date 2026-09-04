//! Reading and writing a linked Google Sheet through the Sheets API v4, using an OAuth access
//! token (`google_auth`). This is the live-sync counterpart to `sheets.rs`'s read-only public CSV
//! export: it matches sheet rows to applications already in the store (by an `Id` column once
//! assigned, or by company/position before it is) and merges the two, instead of blindly creating
//! a new application per row on every read.
//!
//! Merge rule, applied to every tracked field on every sync: a non-blank sheet cell always wins
//! and is pulled into the store; a blank sheet cell is filled from the local value if the local
//! value is not itself blank. No smarter conflict resolution than that (`docs/decisions/
//! google-sync.md`).
//!
//! A write only ever fills a blank cell, corrects a cell in a column this module recognises, or
//! appends a brand-new row — never a column it does not recognise, and never a value derived from
//! nothing.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use serde::Deserialize;

use crate::application::{Application, NewApplication, STATUSES};
use crate::store::{Store, StoreError};

/// The Sheets API v4 base — the production `endpoint` every caller of [`reconcile`] should pass.
/// Tests pass a mock server's URL instead.
pub const SHEETS_API: &str = "https://sheets.googleapis.com/v4/spreadsheets";
/// The range read on every sync: covers a personal tracker's realistic width and depth.
const SHEET_RANGE: &str = "A1:Z1000";
/// A candidate header row must recognise at least this many columns to be trusted.
const HEADING_SCORE_THRESHOLD: usize = 3;

#[derive(Debug, thiserror::Error)]
pub enum GoogleSheetsError {
    #[error("could not reach the sheet")]
    Request(#[source] reqwest::Error),
    #[error("the sheet answered {status}; is the connected account still allowed to edit it?")]
    Denied { status: u16 },
    #[error("could not find a header row with a Company and a Position column")]
    NoHeaderRow,
    #[error("could not read or write the local store")]
    Store(#[from] StoreError),
}

/// A field this module reads from and writes to the sheet. Mirrors `web/src/lib/sheet.ts`'s
/// `COLUMNS`, plus `Id` — duplicated, not shared (ADR-0003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Field {
    Id,
    Company,
    Position,
    Status,
    DateApplied,
    ListingUrl,
    ResumeBranch,
    ResumeDriveLink,
    Notes,
    JdText,
}

const FIELDS: &[(Field, &[&str])] = &[
    (Field::Id, &["id", "hoskinator id"]),
    (
        Field::Company,
        &["company", "employer", "organisation", "organization"],
    ),
    (Field::Position, &["position", "role", "title", "job title"]),
    (Field::Status, &["application status", "status"]),
    (Field::DateApplied, &["date applied", "applied", "date"]),
    (
        // "link" dropped: it is a substring of "Resume Drive Link", which must not also match
        // here.
        Field::ListingUrl,
        &["listing page", "listing", "url", "posting"],
    ),
    (
        // Bare "resume" dropped: it matched any header containing the word (including a future
        // "Resume Drive Link"), which is exactly how an old "Resume" column of PDF filenames
        // once got misread as this field.
        Field::ResumeBranch,
        &["resume branch", "branch"],
    ),
    (
        Field::ResumeDriveLink,
        &["resume drive link", "drive link", "drive"],
    ),
    (Field::Notes, &["notes", "note", "comments"]),
    (
        Field::JdText,
        &["job description", "description", "posting", "jd"],
    ),
];

/// How many of [`FIELDS`] a candidate header row's cells recognise.
fn heading_score(cells: &[String]) -> usize {
    let lowered: Vec<String> = cells.iter().map(|cell| cell.to_lowercase()).collect();
    FIELDS
        .iter()
        .filter(|(_, candidates)| {
            lowered.iter().any(|heading| {
                candidates
                    .iter()
                    .any(|c| heading == c || heading.contains(c))
            })
        })
        .count()
}

/// The index of the field's column in a lowercased header row, if any heading matches.
fn column_for(lowered_header: &[String], candidates: &[&str]) -> Option<usize> {
    lowered_header.iter().position(|heading| {
        candidates
            .iter()
            .any(|c| heading == c || heading.contains(c))
    })
}

fn candidates(field: Field) -> &'static [&'static str] {
    FIELDS
        .iter()
        .find(|(f, _)| *f == field)
        .map(|(_, c)| *c)
        .expect("every Field has an entry in FIELDS")
}

/// Which column (0-indexed) each tracked field lives in, for one sheet.
#[derive(Debug, Clone)]
struct SheetColumns {
    id: Option<usize>,
    company: usize,
    position: usize,
    status: Option<usize>,
    date_applied: Option<usize>,
    listing_url: Option<usize>,
    resume_branch: Option<usize>,
    resume_drive_link: Option<usize>,
    notes: Option<usize>,
    jd_text: Option<usize>,
}

impl SheetColumns {
    fn get(&self, field: Field) -> Option<usize> {
        match field {
            Field::Id => self.id,
            Field::Company => Some(self.company),
            Field::Position => Some(self.position),
            Field::Status => self.status,
            Field::DateApplied => self.date_applied,
            Field::ListingUrl => self.listing_url,
            Field::ResumeBranch => self.resume_branch,
            Field::ResumeDriveLink => self.resume_drive_link,
            Field::Notes => self.notes,
            Field::JdText => self.jd_text,
        }
    }
}

/// Validates one candidate header row and, if it recognises enough columns (including a Company
/// and a Position), returns where each tracked field lives.
fn validate_header(header: &[String]) -> Result<SheetColumns, GoogleSheetsError> {
    if heading_score(header) < HEADING_SCORE_THRESHOLD {
        return Err(GoogleSheetsError::NoHeaderRow);
    }
    let lowered: Vec<String> = header.iter().map(|cell| cell.to_lowercase()).collect();
    let (Some(company), Some(position)) = (
        column_for(&lowered, candidates(Field::Company)),
        column_for(&lowered, candidates(Field::Position)),
    ) else {
        return Err(GoogleSheetsError::NoHeaderRow);
    };
    Ok(SheetColumns {
        id: column_for(&lowered, candidates(Field::Id)),
        company,
        position,
        status: column_for(&lowered, candidates(Field::Status)),
        date_applied: column_for(&lowered, candidates(Field::DateApplied)),
        listing_url: column_for(&lowered, candidates(Field::ListingUrl)),
        resume_branch: column_for(&lowered, candidates(Field::ResumeBranch)),
        resume_drive_link: column_for(&lowered, candidates(Field::ResumeDriveLink)),
        notes: column_for(&lowered, candidates(Field::Notes)),
        jd_text: column_for(&lowered, candidates(Field::JdText)),
    })
}

/// Finds the best-scoring row in a raw sheet read and validates it as a header.
fn find_header_row(rows: &[Vec<String>]) -> Result<(usize, SheetColumns), GoogleSheetsError> {
    let mut best: Option<(usize, usize)> = None;
    for (at, row) in rows.iter().enumerate() {
        let score = heading_score(row);
        if best.is_none_or(|(_, best_score)| score > best_score) {
            best = Some((at, score));
        }
    }
    let (at, _) = best.ok_or(GoogleSheetsError::NoHeaderRow)?;
    let columns = validate_header(&rows[at])?;
    Ok((at, columns))
}

fn cell(row: &[String], column: Option<usize>) -> &str {
    column
        .and_then(|index| row.get(index))
        .map(String::as_str)
        .unwrap_or("")
}

/// A status, case-matched against [`STATUSES`] and written back canonically lowercase.
fn normalize_status(raw: &str) -> String {
    let trimmed = raw.trim();
    STATUSES
        .iter()
        .find(|status| status.eq_ignore_ascii_case(trimmed))
        .map(|status| status.to_string())
        .unwrap_or_else(|| trimmed.to_lowercase())
}

fn application_field(application: &Application, field: Field) -> Option<&str> {
    match field {
        Field::Id => None,
        Field::Company => Some(application.company.as_str()),
        Field::Position => Some(application.position.as_str()),
        Field::Status => Some(application.status.as_str()),
        Field::DateApplied => application.date_applied.as_deref(),
        Field::ListingUrl => application.listing_url.as_deref(),
        Field::ResumeBranch => application.resume_branch.as_deref(),
        Field::ResumeDriveLink => application.resume_drive_link.as_deref(),
        Field::Notes => application.notes.as_deref(),
        Field::JdText => application.jd_text.as_deref(),
    }
}

fn set_application_field(new: &mut NewApplication, field: Field, value: &str) {
    match field {
        Field::Id => {}
        Field::Company => new.company = value.to_string(),
        Field::Position => new.position = value.to_string(),
        Field::Status => new.status = value.to_string(),
        Field::DateApplied => new.date_applied = Some(value.to_string()),
        Field::ListingUrl => new.listing_url = Some(value.to_string()),
        Field::ResumeBranch => new.resume_branch = Some(value.to_string()),
        Field::ResumeDriveLink => new.resume_drive_link = Some(value.to_string()),
        Field::Notes => new.notes = Some(value.to_string()),
        Field::JdText => new.jd_text = Some(value.to_string()),
    }
}

fn new_application_from(fields: &HashMap<Field, String>) -> NewApplication {
    let get = |field: Field| fields.get(&field).cloned().unwrap_or_default();
    let status = get(Field::Status);
    NewApplication {
        company: get(Field::Company),
        position: get(Field::Position),
        status: if status.is_empty() {
            "draft".to_string()
        } else {
            status
        },
        date_applied: fields.get(&Field::DateApplied).cloned(),
        listing_url: fields.get(&Field::ListingUrl).cloned(),
        resume_branch: fields.get(&Field::ResumeBranch).cloned(),
        resume_drive_link: fields.get(&Field::ResumeDriveLink).cloned(),
        notes: fields.get(&Field::Notes).cloned(),
        jd_text: fields.get(&Field::JdText).cloned(),
    }
}

/// One cell to write, in structural terms: `row_offset` counts data rows from the first one
/// (0 = the row right under the header). [`reconcile`] turns this into a real A1 range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellWrite {
    pub column: usize,
    pub row_offset: usize,
    pub value: String,
}

/// A brand-new row to append, in the sheet's column order (blank for any column this module does
/// not recognise or has no value for).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowAppend {
    pub values: Vec<String>,
}

/// What a reconciliation pass found to do. Produced by [`plan_reconciliation`], applied by
/// [`reconcile`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReconciliationPlan {
    pub to_create: Vec<NewApplication>,
    pub to_update: Vec<(i64, NewApplication)>,
    pub cell_writes: Vec<CellWrite>,
    pub row_appends: Vec<RowAppend>,
}

fn column_letter(mut index: usize) -> String {
    let mut letters = Vec::new();
    loop {
        letters.push((b'A' + (index % 26) as u8) as char);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    letters.iter().rev().collect()
}

/// Matches `remote_rows` (everything after the header) against `local`, then works out what
/// needs to change on each side. Pure — no I/O, so this is the part covered by unit tests.
pub fn plan_reconciliation(
    local: &[Application],
    remote_header: &[String],
    remote_rows: &[Vec<String>],
) -> Result<ReconciliationPlan, GoogleSheetsError> {
    let columns = validate_header(remote_header)?;

    let mut by_id: HashMap<i64, &Application> = HashMap::new();
    let mut by_company_position: HashMap<(String, String), Vec<i64>> = HashMap::new();
    for application in local {
        by_id.insert(application.id, application);
        by_company_position
            .entry((application.company.clone(), application.position.clone()))
            .or_default()
            .push(application.id);
    }

    let mut plan = ReconciliationPlan::default();
    let mut matched_local_ids: HashSet<i64> = HashSet::new();

    for (row_offset, row) in remote_rows.iter().enumerate() {
        let id_cell = cell(row, columns.id);
        let id_matched_id = id_cell
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|id| by_id.contains_key(id) && !matched_local_ids.contains(id));

        // A row with neither a company nor a position, and no Id binding it to a known local
        // application, is not an application — a blank row inside the read range, a title/
        // summary line, or trailing sheet padding. Matches `web/src/lib/sheet.ts`'s `parseSheet`
        // filter for the same reason. A row that DOES carry a recognised Id is never skipped this
        // way: otherwise a local application with a blank company and position (a fresh,
        // not-yet-filled-in draft) can never be matched, and gets appended as a "new" row again
        // on every sync, forever.
        if id_matched_id.is_none()
            && cell(row, Some(columns.company)).trim().is_empty()
            && cell(row, Some(columns.position)).trim().is_empty()
        {
            continue;
        }

        let matched_id = id_matched_id.or_else(|| {
            let key = (
                cell(row, Some(columns.company)).trim().to_string(),
                cell(row, Some(columns.position)).trim().to_string(),
            );
            by_company_position
                .get(&key)
                .and_then(|candidates| candidates.iter().find(|id| !matched_local_ids.contains(id)))
                .copied()
        });

        let Some(local_id) = matched_id else {
            // A genuinely new row: nothing local claims it.
            let mut fields = HashMap::new();
            for (field, _) in FIELDS {
                if *field == Field::Id {
                    continue;
                }
                let value = cell(row, columns.get(*field)).trim();
                if !value.is_empty() {
                    let value = if *field == Field::Status {
                        normalize_status(value)
                    } else {
                        value.to_string()
                    };
                    fields.insert(*field, value);
                }
            }
            plan.to_create.push(new_application_from(&fields));
            continue;
        };

        matched_local_ids.insert(local_id);
        let application = by_id[&local_id];
        let mut updated: NewApplication = NewApplication {
            company: application.company.clone(),
            position: application.position.clone(),
            status: application.status.clone(),
            date_applied: application.date_applied.clone(),
            listing_url: application.listing_url.clone(),
            resume_branch: application.resume_branch.clone(),
            resume_drive_link: application.resume_drive_link.clone(),
            notes: application.notes.clone(),
            jd_text: application.jd_text.clone(),
        };
        let mut local_changed = false;

        for (field, _) in FIELDS {
            let field = *field;
            if field == Field::Id {
                if let Some(column) = columns.id
                    && id_cell.trim().is_empty()
                {
                    plan.cell_writes.push(CellWrite {
                        column,
                        row_offset,
                        value: local_id.to_string(),
                    });
                }
                continue;
            }
            let Some(column) = columns.get(field) else {
                continue;
            };
            let sheet_value = cell(row, Some(column)).trim();
            let local_value = application_field(application, field).unwrap_or("").trim();

            // Status is matched case-insensitively and always written back lowercase, on
            // either side, so a sheet that predates the tracker's lowercase convention gets
            // corrected rather than perpetuated.
            if field == Field::Status {
                if sheet_value.is_empty() {
                    if !local_value.is_empty() {
                        plan.cell_writes.push(CellWrite {
                            column,
                            row_offset,
                            value: normalize_status(local_value),
                        });
                    }
                } else {
                    let normalized = normalize_status(sheet_value);
                    if normalized != local_value {
                        set_application_field(&mut updated, field, &normalized);
                        local_changed = true;
                    }
                    if sheet_value != normalized {
                        plan.cell_writes.push(CellWrite {
                            column,
                            row_offset,
                            value: normalized,
                        });
                    }
                }
                continue;
            }

            if !sheet_value.is_empty() && sheet_value != local_value {
                set_application_field(&mut updated, field, sheet_value);
                local_changed = true;
            } else if sheet_value.is_empty() && !local_value.is_empty() {
                plan.cell_writes.push(CellWrite {
                    column,
                    row_offset,
                    value: local_value.to_string(),
                });
            }
        }

        if local_changed {
            plan.to_update.push((local_id, updated));
        }
    }

    for application in local {
        if matched_local_ids.contains(&application.id) {
            continue;
        }
        let width = remote_header
            .len()
            .max(columns.company + 1)
            .max(columns.position + 1);
        let mut values = vec![String::new(); width];
        let mut set = |column: Option<usize>, value: &str| {
            if let Some(index) = column {
                values[index] = value.to_string();
            }
        };
        set(columns.id, &application.id.to_string());
        set(Some(columns.company), &application.company);
        set(Some(columns.position), &application.position);
        set(columns.status, &normalize_status(&application.status));
        set(
            columns.date_applied,
            application.date_applied.as_deref().unwrap_or(""),
        );
        set(
            columns.listing_url,
            application.listing_url.as_deref().unwrap_or(""),
        );
        set(
            columns.resume_branch,
            application.resume_branch.as_deref().unwrap_or(""),
        );
        set(
            columns.resume_drive_link,
            application.resume_drive_link.as_deref().unwrap_or(""),
        );
        set(columns.notes, application.notes.as_deref().unwrap_or(""));
        set(
            columns.jd_text,
            application.jd_text.as_deref().unwrap_or(""),
        );
        plan.row_appends.push(RowAppend { values });
    }

    Ok(plan)
}

/// What one call to [`reconcile`] did.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SyncOutcome {
    pub pulled: usize,
    pub created_locally: usize,
    pub pushed_cells: usize,
    pub appended_to_sheet: usize,
}

/// Reads `spreadsheet_id`, reconciles it against `repository`'s applications, and applies the
/// result to both sides.
pub async fn reconcile(
    store: &Store,
    repository: &str,
    endpoint: &str,
    access_token: &str,
    spreadsheet_id: &str,
) -> Result<SyncOutcome, GoogleSheetsError> {
    let local = store.applications(repository).await?;

    let rows = {
        let endpoint = endpoint.to_string();
        let access_token = access_token.to_string();
        let spreadsheet_id = spreadsheet_id.to_string();
        tokio::task::spawn_blocking(move || {
            read_range(&endpoint, &access_token, &spreadsheet_id, SHEET_RANGE)
        })
        .await
        .expect("reading the sheet should not panic")?
    };

    if rows.is_empty() {
        return Err(GoogleSheetsError::NoHeaderRow);
    }
    let (header_at, columns) = find_header_row(&rows)?;
    let mut header = rows[header_at].clone();
    let data_rows: Vec<Vec<String>> = rows[header_at + 1..].to_vec();

    let mut outcome = SyncOutcome::default();
    // Every phase below is independent — a failure in one (a malformed range, a dropped
    // connection) must not stop the others from running. Each records the first error it hits
    // and carries on, so a partial sync still applies everything it can.
    let mut first_error: Option<GoogleSheetsError> = None;

    // A sheet with no Id column matches every row by company/position, which breaks down the
    // moment two applications share both. Adding the column here means every row past this
    // point matches unambiguously — plan_reconciliation backfills each row's Id the same way it
    // already does for a sheet that had the column but left a row's cell blank.
    if columns.id.is_none() {
        let id_column = header.len();
        let range = format!("{}{}", column_letter(id_column), header_at + 1);
        let result = {
            let endpoint = endpoint.to_string();
            let access_token = access_token.to_string();
            let spreadsheet_id = spreadsheet_id.to_string();
            let value = "Id".to_string();
            tokio::task::spawn_blocking(move || {
                batch_write_cells(&endpoint, &access_token, &spreadsheet_id, &[(range, value)])
            })
            .await
            .expect("writing the sheet should not panic")
        };
        match result {
            Ok(()) => header.push("Id".to_string()),
            Err(error) => drop(first_error.get_or_insert(error)),
        }
    }

    let plan = plan_reconciliation(&local, &header, &data_rows)?;

    for application in &plan.to_create {
        match store.create_application(application, repository).await {
            Ok(_) => outcome.created_locally += 1,
            Err(error) => drop(first_error.get_or_insert(error.into())),
        }
    }
    for (id, application) in &plan.to_update {
        match store.update_application(*id, application).await {
            Ok(_) => outcome.pulled += 1,
            Err(error) => drop(first_error.get_or_insert(error.into())),
        }
    }

    // Appending a local-only application runs before the cell corrections below, so a brand
    // new application always reaches the sheet even if a later write fails. Each row's target is
    // computed explicitly, right after the existing data (see `write_row`).
    let mut next_row = header_at + 2 + data_rows.len();
    for append in &plan.row_appends {
        let endpoint = endpoint.to_string();
        let access_token = access_token.to_string();
        let spreadsheet_id = spreadsheet_id.to_string();
        let values = append.values.clone();
        let row = next_row;
        let result = tokio::task::spawn_blocking(move || {
            write_row(&endpoint, &access_token, &spreadsheet_id, row, &values)
        })
        .await
        .expect("appending to the sheet should not panic");
        match result {
            Ok(()) => {
                outcome.appended_to_sheet += 1;
                next_row += 1;
            }
            Err(error) => drop(first_error.get_or_insert(error)),
        }
    }

    if !plan.cell_writes.is_empty() {
        let writes: Vec<(String, String)> = plan
            .cell_writes
            .iter()
            .map(|write| {
                let range = format!(
                    "{}{}",
                    column_letter(write.column),
                    header_at + 2 + write.row_offset
                );
                (range, write.value.clone())
            })
            .collect();
        let endpoint = endpoint.to_string();
        let access_token = access_token.to_string();
        let spreadsheet_id = spreadsheet_id.to_string();
        let result = tokio::task::spawn_blocking(move || {
            batch_write_cells(&endpoint, &access_token, &spreadsheet_id, &writes)
        })
        .await
        .expect("writing the sheet should not panic");
        match result {
            Ok(()) => outcome.pushed_cells = plan.cell_writes.len(),
            Err(error) => drop(first_error.get_or_insert(error)),
        }
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(outcome),
    }
}

/// Clears a row's tracked-column cells from the linked sheet — called when an application is
/// deleted locally, so the next sync does not read its old row back as a brand-new application.
/// Matched by `application_id` against the Id column when the row has one, falling back to
/// company and position for a row that predates it, and clearing only the first such match. A
/// no-op if nothing matches, including an unrecognisable header: deleting locally must never fail
/// because the sheet side had nothing to clean up.
pub async fn remove_from_sheet(
    endpoint: &str,
    access_token: &str,
    spreadsheet_id: &str,
    application_id: i64,
    company: &str,
    position: &str,
) -> Result<(), GoogleSheetsError> {
    let rows = {
        let endpoint = endpoint.to_string();
        let access_token = access_token.to_string();
        let spreadsheet_id = spreadsheet_id.to_string();
        tokio::task::spawn_blocking(move || {
            read_range(&endpoint, &access_token, &spreadsheet_id, SHEET_RANGE)
        })
        .await
        .expect("reading the sheet should not panic")?
    };
    let Ok((header_at, columns)) = find_header_row(&rows) else {
        return Ok(());
    };

    let company = company.trim();
    let position = position.trim();
    let data_rows = &rows[header_at + 1..];

    let by_id = columns.id.and_then(|id_column| {
        data_rows
            .iter()
            .position(|row| cell(row, Some(id_column)).trim().parse::<i64>() == Ok(application_id))
    });
    let Some(matched_offset) = by_id.or_else(|| {
        data_rows.iter().position(|row| {
            cell(row, Some(columns.company)).trim() == company
                && cell(row, Some(columns.position)).trim() == position
        })
    }) else {
        return Ok(());
    };

    let tracked_columns = [
        columns.id,
        Some(columns.company),
        Some(columns.position),
        columns.status,
        columns.date_applied,
        columns.listing_url,
        columns.resume_branch,
        columns.notes,
        columns.jd_text,
    ];
    let absolute_row = header_at + 2 + matched_offset;
    let ranges: Vec<String> = tracked_columns
        .into_iter()
        .flatten()
        .map(|column| format!("{}{}", column_letter(column), absolute_row))
        .collect();

    let endpoint = endpoint.to_string();
    let access_token = access_token.to_string();
    let spreadsheet_id = spreadsheet_id.to_string();
    tokio::task::spawn_blocking(move || {
        clear_cells(&endpoint, &access_token, &spreadsheet_id, &ranges)
    })
    .await
    .expect("clearing the sheet should not panic")
}

/// Clears cells (`values:batchClear`) — used to remove a deleted application's row.
fn clear_cells(
    endpoint: &str,
    access_token: &str,
    spreadsheet_id: &str,
    ranges: &[String],
) -> Result<(), GoogleSheetsError> {
    let url = format!("{endpoint}/{spreadsheet_id}/values:batchClear");
    let body = serde_json::json!({ "ranges": ranges });
    let response = client()?
        .post(&url)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .map_err(GoogleSheetsError::Request)?;
    if !response.status().is_success() {
        return Err(GoogleSheetsError::Denied {
            status: response.status().as_u16(),
        });
    }
    Ok(())
}

fn client() -> Result<reqwest::blocking::Client, GoogleSheetsError> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(GoogleSheetsError::Request)
}

#[derive(Deserialize)]
struct ValueRange {
    #[serde(default)]
    values: Vec<Vec<String>>,
}

/// Reads a range as a 2D grid of strings. A short row (a trailing blank cell Sheets omits) is
/// left short — callers index through [`cell`], which treats a missing index as blank.
fn read_range(
    endpoint: &str,
    access_token: &str,
    spreadsheet_id: &str,
    range: &str,
) -> Result<Vec<Vec<String>>, GoogleSheetsError> {
    let url = format!("{endpoint}/{spreadsheet_id}/values/{range}");
    let response = client()?
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .map_err(GoogleSheetsError::Request)?;
    if !response.status().is_success() {
        return Err(GoogleSheetsError::Denied {
            status: response.status().as_u16(),
        });
    }
    let payload: ValueRange = response.json().map_err(GoogleSheetsError::Request)?;
    Ok(payload.values)
}

/// Writes several individual cells in one request (`values:batchUpdate`).
fn batch_write_cells(
    endpoint: &str,
    access_token: &str,
    spreadsheet_id: &str,
    writes: &[(String, String)],
) -> Result<(), GoogleSheetsError> {
    let url = format!("{endpoint}/{spreadsheet_id}/values:batchUpdate");
    let data: Vec<serde_json::Value> = writes
        .iter()
        .map(|(range, value)| {
            serde_json::json!({
                "range": range,
                "values": [[value]],
            })
        })
        .collect();
    let body = serde_json::json!({
        "valueInputOption": "USER_ENTERED",
        "data": data,
    });
    let response = client()?
        .post(&url)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .map_err(GoogleSheetsError::Request)?;
    if !response.status().is_success() {
        return Err(GoogleSheetsError::Denied {
            status: response.status().as_u16(),
        });
    }
    Ok(())
}

/// Writes one full row at an explicit row number computed by the caller — never through Sheets'
/// own table-autodetection (`values:append`). That heuristic locates "the table" to extend by
/// scanning the given range for existing data; once a single stray value exists somewhere far
/// off within it, every future append can lock onto that instead of the real table and keep
/// landing there. That is what corrupted the sheet with hundreds of stray rows in the past — an
/// explicit target removes the guesswork entirely.
fn write_row(
    endpoint: &str,
    access_token: &str,
    spreadsheet_id: &str,
    row: usize,
    values: &[String],
) -> Result<(), GoogleSheetsError> {
    let range = format!(
        "A{row}:{}{row}",
        column_letter(values.len().saturating_sub(1))
    );
    let url = format!("{endpoint}/{spreadsheet_id}/values/{range}?valueInputOption=USER_ENTERED");
    let body = serde_json::json!({ "values": [values] });
    let response = client()?
        .put(&url)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .map_err(GoogleSheetsError::Request)?;
    if !response.status().is_success() {
        return Err(GoogleSheetsError::Denied {
            status: response.status().as_u16(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn application(id: i64, company: &str, position: &str) -> Application {
        Application {
            id,
            company: company.to_string(),
            position: position.to_string(),
            status: "applied".to_string(),
            date_applied: None,
            listing_url: None,
            resume_branch: None,
            resume_drive_link: None,
            notes: None,
            jd_text: None,
            created_at: "2026-01-01".to_string(),
        }
    }

    fn row(cells: &[&str]) -> Vec<String> {
        cells.iter().map(|c| c.to_string()).collect()
    }

    const HEADER: &[&str] = &[
        "Id",
        "Company",
        "Position",
        "Status",
        "Date Applied",
        "Notes",
    ];

    #[test]
    fn a_row_with_a_matching_id_pulls_its_sheet_status_into_the_local_application() {
        let local = vec![application(1, "Acme", "Engineer")];
        let header = row(HEADER);
        let remote = vec![row(&["1", "Acme", "Engineer", "interview", "", ""])];

        let plan = plan_reconciliation(&local, &header, &remote).unwrap();

        assert_eq!(
            plan.to_update,
            vec![(
                1,
                NewApplication {
                    company: "Acme".to_string(),
                    position: "Engineer".to_string(),
                    status: "interview".to_string(),
                    date_applied: None,
                    listing_url: None,
                    resume_branch: None,
                    resume_drive_link: None,
                    notes: None,
                    jd_text: None,
                },
            )]
        );
        assert!(plan.to_create.is_empty());
    }

    #[test]
    fn a_capitalized_sheet_status_is_lowercased_locally_and_corrected_on_the_sheet() {
        let mut local_application = application(1, "Acme", "Engineer");
        local_application.status = "interview".to_string();
        let local = vec![local_application];
        let header = row(HEADER);
        let remote = vec![row(&["1", "Acme", "Engineer", "Interview", "", ""])];

        let plan = plan_reconciliation(&local, &header, &remote).unwrap();

        // Already "interview" locally — sheet's "Interview" carries no new information, so no
        // store write, only the sheet's own casing gets corrected.
        assert!(plan.to_update.is_empty());
        assert_eq!(
            plan.cell_writes,
            vec![CellWrite {
                column: 3,
                row_offset: 0,
                value: "interview".to_string(),
            }]
        );
    }

    #[test]
    fn a_capitalized_local_status_is_lowercased_when_pushed_into_a_blank_sheet_cell() {
        let mut local_application = application(1, "Acme", "Engineer");
        local_application.status = "Interview".to_string();
        let local = vec![local_application];
        let header = row(HEADER);
        let remote = vec![row(&["1", "Acme", "Engineer", "", "", ""])];

        let plan = plan_reconciliation(&local, &header, &remote).unwrap();

        assert_eq!(
            plan.cell_writes,
            vec![CellWrite {
                column: 3,
                row_offset: 0,
                value: "interview".to_string(),
            }]
        );
    }

    #[test]
    fn a_new_local_only_application_is_appended_with_a_lowercase_status() {
        let mut local_application = application(9, "Acme", "Engineer");
        local_application.status = "Interview".to_string();
        let local = vec![local_application];
        let header = row(HEADER);

        let plan = plan_reconciliation(&local, &header, &[]).unwrap();

        assert_eq!(plan.row_appends[0].values[3], "interview");
    }

    #[test]
    fn a_blank_id_row_matches_by_company_and_position_and_gets_the_id_written_back() {
        let local = vec![application(7, "Initech", "PM")];
        let header = row(HEADER);
        let remote = vec![row(&["", "Initech", "PM", "applied", "", ""])];

        let plan = plan_reconciliation(&local, &header, &remote).unwrap();

        assert!(plan.to_create.is_empty());
        assert_eq!(
            plan.cell_writes,
            vec![CellWrite {
                column: 0,
                row_offset: 0,
                value: "7".to_string(),
            }]
        );
    }

    #[test]
    fn a_row_matching_nothing_becomes_a_new_local_application() {
        let header = row(HEADER);
        let remote = vec![row(&["", "NewCo", "Designer", "draft", "", ""])];

        let plan = plan_reconciliation(&[], &header, &remote).unwrap();

        assert_eq!(plan.to_create.len(), 1);
        assert_eq!(plan.to_create[0].company, "NewCo");
        assert_eq!(plan.to_create[0].position, "Designer");
    }

    #[test]
    fn a_row_with_no_company_or_position_is_not_created_as_an_application() {
        let header = row(HEADER);
        let remote = vec![
            row(&["", "", "", "", "", ""]),
            row(&["", "", "", "draft", "", "a stray note"]),
        ];

        let plan = plan_reconciliation(&[], &header, &remote).unwrap();

        assert!(plan.to_create.is_empty());
    }

    #[test]
    fn a_local_application_absent_from_the_sheet_is_appended() {
        let local = vec![application(3, "LocalOnly", "Dev")];
        let header = row(HEADER);

        let plan = plan_reconciliation(&local, &header, &[]).unwrap();

        assert_eq!(plan.row_appends.len(), 1);
        assert_eq!(plan.row_appends[0].values[0], "3");
        assert_eq!(plan.row_appends[0].values[1], "LocalOnly");
        assert_eq!(plan.row_appends[0].values[2], "Dev");
    }

    #[test]
    fn a_blank_sheet_cell_is_filled_from_the_local_value_rather_than_overwritten() {
        let mut local_application = application(1, "Acme", "Engineer");
        local_application.notes = Some("referred by Jo".to_string());
        let local = vec![local_application];
        let header = row(HEADER);
        let remote = vec![row(&["1", "Acme", "Engineer", "applied", "", ""])];

        let plan = plan_reconciliation(&local, &header, &remote).unwrap();

        assert!(plan.to_update.is_empty());
        assert_eq!(
            plan.cell_writes,
            vec![CellWrite {
                column: 5,
                row_offset: 0,
                value: "referred by Jo".to_string(),
            }]
        );
    }

    #[test]
    fn a_populated_sheet_cell_overwrites_a_different_local_value_silently() {
        let mut local_application = application(1, "Acme", "Engineer");
        local_application.notes = Some("old note".to_string());
        let local = vec![local_application];
        let header = row(HEADER);
        let remote = vec![row(&["1", "Acme", "Engineer", "applied", "", "new note"])];

        let plan = plan_reconciliation(&local, &header, &remote).unwrap();

        assert_eq!(plan.to_update[0].1.notes, Some("new note".to_string()));
    }

    #[test]
    fn a_header_row_with_no_company_or_position_column_is_rejected() {
        let header = row(&["Notes", "Comments"]);

        let error = plan_reconciliation(&[], &header, &[]).unwrap_err();

        assert!(matches!(error, GoogleSheetsError::NoHeaderRow));
    }

    #[test]
    fn column_letters_wrap_past_z() {
        assert_eq!(column_letter(0), "A");
        assert_eq!(column_letter(25), "Z");
        assert_eq!(column_letter(26), "AA");
        assert_eq!(column_letter(27), "AB");
    }

    #[tokio::test]
    async fn a_successful_read_is_parsed_into_rows() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sheet-1/values/A1:B2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "values": [["Company", "Position"], ["Acme", "Engineer"]],
            })))
            .mount(&server)
            .await;

        let endpoint = server.uri();
        let rows =
            tokio::task::spawn_blocking(move || read_range(&endpoint, "token", "sheet-1", "A1:B2"))
                .await
                .unwrap()
                .unwrap();

        assert_eq!(
            rows,
            vec![
                vec!["Company".to_string(), "Position".to_string()],
                vec!["Acme".to_string(), "Engineer".to_string()],
            ]
        );
    }

    #[tokio::test]
    async fn a_denied_read_reports_the_status() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sheet-1/values/A1:B2"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let endpoint = server.uri();
        let error =
            tokio::task::spawn_blocking(move || read_range(&endpoint, "token", "sheet-1", "A1:B2"))
                .await
                .unwrap()
                .unwrap_err();

        assert!(matches!(error, GoogleSheetsError::Denied { status: 403 }));
    }

    #[tokio::test]
    async fn remove_from_sheet_clears_a_matching_rows_tracked_cells() {
        use wiremock::matchers::{body_json, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sheet-1/values/A1:Z1000"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "values": [HEADER, ["1", "Acme", "Engineer", "applied", "", "a note"]],
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/sheet-1/values:batchClear"))
            .and(body_json(serde_json::json!({
                "ranges": ["A2", "B2", "C2", "D2", "E2", "F2"],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let endpoint = server.uri();
        remove_from_sheet(&endpoint, "token", "sheet-1", 1, "Acme", "Engineer")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn remove_from_sheet_falls_back_to_company_and_position_for_a_blank_id_cell() {
        use wiremock::matchers::{body_json, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sheet-1/values/A1:Z1000"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "values": [HEADER, ["", "Acme", "Engineer", "applied", "", "a note"]],
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/sheet-1/values:batchClear"))
            .and(body_json(serde_json::json!({
                "ranges": ["A2", "B2", "C2", "D2", "E2", "F2"],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let endpoint = server.uri();
        remove_from_sheet(&endpoint, "token", "sheet-1", 1, "Acme", "Engineer")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn remove_from_sheet_is_a_no_op_when_nothing_matches() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sheet-1/values/A1:Z1000"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "values": [HEADER, ["1", "Acme", "Engineer", "applied", "", ""]],
            })))
            .mount(&server)
            .await;
        // No `values:batchClear` mock mounted — a call to it would 404, so a passing test
        // proves `remove_from_sheet` never made one.

        let endpoint = server.uri();
        remove_from_sheet(&endpoint, "token", "sheet-1", 99, "Initech", "PM")
            .await
            .unwrap();
    }

    async fn open_temp_store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("store.db")).await.unwrap();
        (dir, store)
    }

    #[tokio::test]
    async fn reconcile_adds_an_id_column_to_a_sheet_that_has_none() {
        use wiremock::matchers::{body_json, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let (_dir, store) = open_temp_store().await;
        let application = store
            .create_application(
                &NewApplication {
                    company: "Acme".to_string(),
                    position: "Engineer".to_string(),
                    status: "applied".to_string(),
                    date_applied: None,
                    listing_url: None,
                    resume_branch: None,
                    resume_drive_link: None,
                    notes: None,
                    jd_text: None,
                },
                "owner/repo",
            )
            .await
            .unwrap();

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sheet-1/values/A1:Z1000"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "values": [
                    ["Company", "Position", "Status"],
                    ["Acme", "Engineer", "applied"],
                ],
            })))
            .mount(&server)
            .await;
        // The header-cell write (adding "Id") and the row's backfilled Id both go through
        // batchUpdate — assert on the header-cell one specifically; a mismatched call 404s.
        Mock::given(method("POST"))
            .and(path("/sheet-1/values:batchUpdate"))
            .and(body_json(serde_json::json!({
                "valueInputOption": "USER_ENTERED",
                "data": [{"range": "D1", "values": [["Id"]]}],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/sheet-1/values:batchUpdate"))
            .and(body_json(serde_json::json!({
                "valueInputOption": "USER_ENTERED",
                "data": [{"range": "D2", "values": [[application.id.to_string()]]}],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let endpoint = server.uri();
        let outcome = reconcile(&store, "owner/repo", &endpoint, "token", "sheet-1")
            .await
            .unwrap();

        assert_eq!(outcome.pushed_cells, 1);
    }

    #[tokio::test]
    async fn reconcile_appends_a_local_only_application_at_an_explicit_row() {
        // Regression test: a local-only application used to be appended through Sheets'
        // `values:append`, which locates "the table" to extend by scanning the whole read range
        // for existing data — a single stray value far off in that range can make every future
        // append lock onto it instead of the real table. No mock is mounted for that endpoint
        // here, so a call to it 404s and fails the test; only an explicit, computed target row
        // is accepted.
        use wiremock::matchers::{body_json, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let (_dir, store) = open_temp_store().await;
        let created = store
            .create_application(
                &NewApplication {
                    company: "NewCo".to_string(),
                    position: "Dev".to_string(),
                    status: "applied".to_string(),
                    date_applied: None,
                    listing_url: None,
                    resume_branch: None,
                    resume_drive_link: None,
                    notes: None,
                    jd_text: None,
                },
                "owner/repo",
            )
            .await
            .unwrap();

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sheet-1/values/A1:Z1000"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "values": [
                    HEADER,
                    ["999", "Acme", "Engineer", "applied", "", ""],
                ],
            })))
            .mount(&server)
            .await;
        // Id 999 does not match the newly created application's own id, so this seeded row is
        // untouched — it exists only to give the sheet one pre-existing data row, so the target
        // row for the append below is meaningfully past the header rather than immediately below
        // it. The header row is at sheet row 1, one existing data row follows it, so the next
        // row is 3 — never the ambiguous whole-range append.
        Mock::given(method("PUT"))
            .and(path("/sheet-1/values/A3:F3"))
            .and(body_json(serde_json::json!({
                "values": [[created.id.to_string(), "NewCo", "Dev", "applied", "", ""]],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let endpoint = server.uri();
        let outcome = reconcile(&store, "owner/repo", &endpoint, "token", "sheet-1")
            .await
            .unwrap();

        assert_eq!(outcome.appended_to_sheet, 1);
    }

    #[test]
    fn a_blank_company_and_position_row_still_matches_its_known_id() {
        // Regression test: a local application with a blank company and position (a fresh,
        // not-yet-filled-in draft) has a sheet row that is also blank in company and position —
        // it must still be recognised as that application via its Id column, not treated as
        // padding and re-appended as a "new" row on every sync.
        let local = vec![application(20, "", "")];
        let header = row(HEADER);
        let remote = vec![row(&["20", "", "", "draft", "", ""])];

        let plan = plan_reconciliation(&local, &header, &remote).unwrap();

        assert!(plan.to_create.is_empty());
        assert!(plan.row_appends.is_empty());
    }
}
