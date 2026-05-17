use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::Subcommand;
use reqwest::blocking::multipart;
use serde_json::Value;
use tracing::{debug, info};

use crate::client::WfClient;
use crate::config::Config;
use crate::models::{
    ActivityBulkMutationRequest, ActivityBulkMutationResult, ActivityImport, ActivitySearchRequest,
    ActivitySearchResponse, ImportCheckBody, ImportParseResponse, ImportResponse, NewActivity,
};

#[derive(Debug, Subcommand)]
pub enum ActivityCmd {
    /// Parse a CSV server-side without writing anything. Useful as a
    /// dry-run of the file shape before invoking `import-check`.
    ImportParse {
        /// Target account id (server uses it to look up the
        /// account-specific column mapping).
        #[arg(long)]
        account: String,
        /// Path to the CSV file.
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Validate a CSV import end-to-end (parse + map + check). Doesn't
    /// write rows; surfaces per-row errors and warnings.
    ImportCheck {
        #[arg(long)]
        account: String,
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Upload + commit a CSV import. Writes activities to the account.
    /// Duplicates (matching the server's content hash on date / type /
    /// amount / currency / comment) are silently skipped.
    Import {
        #[arg(long)]
        account: String,
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Bulk-create activities from a JSON file. The file must be either
    /// a JSON array of `NewActivity` objects or an
    /// `ActivityBulkMutationRequest` (`{creates, updates, deleteIds}`).
    /// Idempotency is keyed on `sourceRecordId` (plus content) — supply
    /// it for stable re-runs. Note: any single duplicate fails the
    /// whole batch (HTTP 400); pre-filter via `activities search`.
    BulkCreate {
        /// Path to the JSON file.
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// List activities (page-based; 0-indexed pages).
    Search {
        #[arg(long)]
        account: Option<String>,
        #[arg(long, default_value_t = 0)]
        page: i64,
        #[arg(long = "page-size", default_value_t = 50)]
        page_size: i64,
        #[arg(long = "date-from")]
        date_from: Option<String>,
        #[arg(long = "date-to")]
        date_to: Option<String>,
        #[arg(long)]
        json: bool,
    },

    /// Delete a single activity by id.
    Delete { id: String },
}

pub fn run(cfg: &Config, cmd: &ActivityCmd) -> Result<()> {
    match cmd {
        ActivityCmd::ImportParse {
            account,
            file,
            json,
        } => import_parse(cfg, account, file, *json),
        ActivityCmd::ImportCheck {
            account,
            file,
            json,
        } => import_check(cfg, account, file, *json),
        ActivityCmd::Import {
            account,
            file,
            json,
        } => import_commit(cfg, account, file, *json),
        ActivityCmd::BulkCreate { file, json } => bulk_create(cfg, file, *json),
        ActivityCmd::Search {
            account,
            page,
            page_size,
            date_from,
            date_to,
            json,
        } => search(
            cfg,
            account.as_deref(),
            *page,
            *page_size,
            date_from.as_deref(),
            date_to.as_deref(),
            *json,
        ),
        ActivityCmd::Delete { id } => delete(cfg, id),
    }
}

// ─────────────────────────────────────────────────────────────────────
// CSV → ActivityImport mapping
// ─────────────────────────────────────────────────────────────────────

/// Map a parsed CSV (server-returned rows) into [`ActivityImport`]
/// values using case-insensitive header matching against the known
/// JSON field names. Each row inherits `account_id` from the caller.
///
/// Recognized headers (case-insensitive, `snake_case` and `camelCase`
/// both accepted):
///
/// * `date` *(required)*
/// * `activityType` / `activity_type` *(required)*
/// * `amount`, `currency`
/// * `symbol`, `quantity`, `unitPrice`, `fee`, `comment`, `subtype`,
///   `fxRate`, `id`
///
/// Unknown columns are ignored. Empty cells become `None`. Trailing
/// `\r` (from CRLF CSVs) is stripped from every cell.
pub fn map_csv_to_activities(
    headers: &[String],
    rows: &[Vec<String>],
    account_id: &str,
) -> Result<Vec<ActivityImport>> {
    let mut idx = ColumnIndex::default();
    for (i, h) in headers.iter().enumerate() {
        idx.assign(h, i);
    }
    let date_col = idx
        .date
        .ok_or_else(|| anyhow!("CSV missing 'date' column"))?;
    let type_col = idx
        .activity_type
        .ok_or_else(|| anyhow!("CSV missing 'activityType' column"))?;

    let cell = |row: &Vec<String>, col: usize| -> Option<String> {
        let raw = row.get(col)?.trim_end_matches('\r').trim().to_string();
        if raw.is_empty() {
            None
        } else {
            Some(raw)
        }
    };

    let mut out = Vec::with_capacity(rows.len());
    for (n, row) in rows.iter().enumerate() {
        let date = cell(row, date_col)
            .ok_or_else(|| anyhow!("row {} (1-indexed): empty 'date'", n + 1))?;
        let activity_type = cell(row, type_col)
            .ok_or_else(|| anyhow!("row {} (1-indexed): empty 'activityType'", n + 1))?;
        let symbol = idx.symbol.and_then(|c| cell(row, c)).unwrap_or_default();
        let currency = idx.currency.and_then(|c| cell(row, c)).unwrap_or_default();

        out.push(ActivityImport {
            id: idx.id.and_then(|c| cell(row, c)),
            date,
            symbol,
            activity_type,
            quantity: idx.quantity.and_then(|c| cell(row, c)),
            unit_price: idx.unit_price.and_then(|c| cell(row, c)),
            currency,
            fee: idx.fee.and_then(|c| cell(row, c)),
            amount: idx.amount.and_then(|c| cell(row, c)),
            comment: idx.comment.and_then(|c| cell(row, c)),
            account_id: Some(account_id.to_string()),
            subtype: idx.subtype.and_then(|c| cell(row, c)),
            fx_rate: idx.fx_rate.and_then(|c| cell(row, c)),
            is_draft: false,
            is_valid: true,
            errors: None,
            warnings: None,
            duplicate_of_id: None,
        });
    }
    Ok(out)
}

/// Parse a `bulk-create` input. Accepts either a top-level JSON array
/// of `NewActivity` (which is wrapped into a `creates` slot) or a full
/// `ActivityBulkMutationRequest` object. Errors on scalar input or on
/// an empty effective payload.
fn parse_bulk_body(raw: &str) -> Result<ActivityBulkMutationRequest> {
    let value: Value = serde_json::from_str(raw).context("invalid JSON")?;
    let body = match value {
        Value::Array(_) => {
            let creates: Vec<NewActivity> =
                serde_json::from_value(value).context("decode JSON array as [NewActivity]")?;
            ActivityBulkMutationRequest {
                creates,
                updates: vec![],
                delete_ids: vec![],
            }
        }
        Value::Object(_) => {
            serde_json::from_value(value).context("decode object as ActivityBulkMutationRequest")?
        }
        _ => return Err(anyhow!("expected JSON array or object, got scalar")),
    };
    if body.creates.is_empty() && body.updates.is_empty() && body.delete_ids.is_empty() {
        return Err(anyhow!(
            "nothing to do: creates/updates/deleteIds all empty"
        ));
    }
    Ok(body)
}

#[derive(Default)]
struct ColumnIndex {
    id: Option<usize>,
    date: Option<usize>,
    symbol: Option<usize>,
    activity_type: Option<usize>,
    quantity: Option<usize>,
    unit_price: Option<usize>,
    currency: Option<usize>,
    fee: Option<usize>,
    amount: Option<usize>,
    comment: Option<usize>,
    subtype: Option<usize>,
    fx_rate: Option<usize>,
}

impl ColumnIndex {
    fn assign(&mut self, header: &str, position: usize) {
        let norm = header.trim().to_ascii_lowercase().replace('_', "");
        match norm.as_str() {
            "id" => self.id = Some(position),
            "date" => self.date = Some(position),
            "symbol" => self.symbol = Some(position),
            "activitytype" | "type" => self.activity_type = Some(position),
            "quantity" | "qty" => self.quantity = Some(position),
            "unitprice" | "price" => self.unit_price = Some(position),
            "currency" => self.currency = Some(position),
            "fee" => self.fee = Some(position),
            "amount" => self.amount = Some(position),
            "comment" | "notes" | "memo" => self.comment = Some(position),
            "subtype" => self.subtype = Some(position),
            "fxrate" => self.fx_rate = Some(position),
            _ => {}
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// Command implementations
// ─────────────────────────────────────────────────────────────────────

fn parse_csv_via_server(
    client: &WfClient,
    account_id: &str,
    file: &Path,
) -> Result<ImportParseResponse> {
    let form = multipart::Form::new()
        .text("accountId", account_id.to_string())
        .file("file", file)
        .with_context(|| format!("attach {}", file.display()))?;
    debug!(file = %file.display(), "POST /api/v1/activities/import/parse (multipart)");
    client.post_multipart::<ImportParseResponse>("/api/v1/activities/import/parse", form)
}

fn import_parse(cfg: &Config, account: &str, file: &Path, emit_json: bool) -> Result<()> {
    let client = WfClient::new(cfg)?;
    let resp = parse_csv_via_server(&client, account, file)?;
    print_parse(&resp, emit_json)
}

fn import_check(cfg: &Config, account: &str, file: &Path, emit_json: bool) -> Result<()> {
    let client = WfClient::new(cfg)?;
    let parsed = parse_csv_via_server(&client, account, file)?;
    let activities = map_csv_to_activities(&parsed.headers, &parsed.rows, account)?;
    info!(
        rows = activities.len(),
        "POST /api/v1/activities/import/check"
    );
    let body = ImportCheckBody {
        activities: &activities,
    };
    let validated: Vec<ActivityImport> =
        client.post_json("/api/v1/activities/import/check", &body)?;
    print_check(&validated, emit_json)
}

fn import_commit(cfg: &Config, account: &str, file: &Path, emit_json: bool) -> Result<()> {
    let client = WfClient::new(cfg)?;
    let parsed = parse_csv_via_server(&client, account, file)?;
    let activities = map_csv_to_activities(&parsed.headers, &parsed.rows, account)?;
    info!(rows = activities.len(), "POST /api/v1/activities/import");
    let body = ImportCheckBody {
        activities: &activities,
    };
    let resp: ImportResponse = client.post_json("/api/v1/activities/import", &body)?;
    if emit_json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        print_import_summary(&resp, activities.len());
    }
    Ok(())
}

fn print_import_summary(resp: &ImportResponse, sent: usize) {
    let summary = resp.get("summary");
    let total = summary.and_then(|s| s.get("total")).and_then(Value::as_u64);
    let imported = summary
        .and_then(|s| s.get("imported"))
        .and_then(Value::as_u64);
    let skipped = summary
        .and_then(|s| s.get("skipped"))
        .and_then(Value::as_u64);
    let duplicates = summary
        .and_then(|s| s.get("duplicates"))
        .and_then(Value::as_u64);
    match (total, imported, skipped, duplicates) {
        (Some(t), Some(i), Some(s), Some(d)) => {
            println!("import: total={t} imported={i} skipped={s} duplicates={d}")
        }
        _ => println!("import: ok ({sent} rows sent)"),
    }
}

fn bulk_create(cfg: &Config, file: &Path, emit_json: bool) -> Result<()> {
    let raw = std::fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
    let body = parse_bulk_body(&raw).with_context(|| format!("parse {}", file.display()))?;
    let client = WfClient::new(cfg)?;
    info!(
        creates = body.creates.len(),
        updates = body.updates.len(),
        deletes = body.delete_ids.len(),
        "POST /api/v1/activities/bulk"
    );
    let result: ActivityBulkMutationResult = client.post_json("/api/v1/activities/bulk", &body)?;
    if emit_json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_bulk_summary(&result);
    }
    Ok(())
}

fn search(
    cfg: &Config,
    account: Option<&str>,
    page: i64,
    page_size: i64,
    date_from: Option<&str>,
    date_to: Option<&str>,
    emit_json: bool,
) -> Result<()> {
    let client = WfClient::new(cfg)?;
    let body = ActivitySearchRequest {
        page,
        page_size,
        account_id_filter: account.map(str::to_string),
        activity_type_filter: None,
        date_from: date_from.map(str::to_string),
        date_to: date_to.map(str::to_string),
    };
    let resp: ActivitySearchResponse = client.post_json("/api/v1/activities/search", &body)?;
    if emit_json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
        return Ok(());
    }
    print_search(&resp);
    Ok(())
}

fn delete(cfg: &Config, id: &str) -> Result<()> {
    let client = WfClient::new(cfg)?;
    client.delete(&format!("/api/v1/activities/{id}"))?;
    println!("deleted: {id}");
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────
// Output helpers
// ─────────────────────────────────────────────────────────────────────

fn print_parse(resp: &ImportParseResponse, emit_json: bool) -> Result<()> {
    if emit_json {
        println!("{}", serde_json::to_string_pretty(resp)?);
        return Ok(());
    }
    println!("rows detected : {}", resp.row_count);
    println!("headers       : {}", resp.headers.join(", "));
    println!("errors        : {}", resp.errors.len());
    for e in &resp.errors {
        println!("  - {e}");
    }
    if !resp.rows.is_empty() {
        println!("preview (first 3 rows):");
        for row in resp.rows.iter().take(3) {
            println!("  {}", row.join(" | "));
        }
    }
    Ok(())
}

fn print_check(rows: &[ActivityImport], emit_json: bool) -> Result<()> {
    if emit_json {
        println!("{}", serde_json::to_string_pretty(rows)?);
        return Ok(());
    }
    let valid = rows.iter().filter(|r| r.is_valid).count();
    let with_errors = rows.iter().filter(|r| r.errors.is_some()).count();
    let with_warnings = rows.iter().filter(|r| r.warnings.is_some()).count();
    let with_duplicates = rows.iter().filter(|r| r.duplicate_of_id.is_some()).count();
    println!("check: {valid}/{} valid", rows.len());
    println!("  errors     : {with_errors}");
    println!("  warnings   : {with_warnings}");
    println!("  duplicates : {with_duplicates}");
    for (i, row) in rows.iter().enumerate() {
        if let Some(errs) = &row.errors {
            println!(
                "  row {} ({} {} {}): errors = {}",
                i + 1,
                row.date,
                row.activity_type,
                row.amount.clone().unwrap_or_default(),
                errs
            );
        }
        if let Some(dup_of) = &row.duplicate_of_id {
            println!(
                "  row {} ({} {} {}): duplicate of {}",
                i + 1,
                row.date,
                row.activity_type,
                row.amount.clone().unwrap_or_default(),
                dup_of
            );
        }
    }
    Ok(())
}

fn print_bulk_summary(result: &ActivityBulkMutationResult) {
    let created = result
        .get("created")
        .and_then(Value::as_array)
        .map(Vec::len);
    let updated = result
        .get("updated")
        .and_then(Value::as_array)
        .map(Vec::len);
    let deleted = result
        .get("deleted")
        .and_then(Value::as_array)
        .map(Vec::len);
    let errors = result.get("errors").and_then(Value::as_array).map(Vec::len);
    println!(
        "bulk: created={} updated={} deleted={} errors={}",
        created.unwrap_or(0),
        updated.unwrap_or(0),
        deleted.unwrap_or(0),
        errors.unwrap_or(0)
    );
    if let Some(errs) = result.get("errors").and_then(Value::as_array) {
        for e in errs.iter().take(10) {
            println!("  - {e}");
        }
    }
}

fn print_search(resp: &ActivitySearchResponse) {
    let total = resp
        .pointer("/meta/totalRowCount")
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    let data = resp.get("data").and_then(Value::as_array);
    let rows = data.map(Vec::as_slice).unwrap_or(&[]);
    println!("{} rows (total {total})", rows.len());
    if rows.is_empty() {
        return;
    }
    println!(
        "{:<10}  {:<12}  {:>14}  {:<6}  COMMENT",
        "DATE", "TYPE", "AMOUNT", "CCY"
    );
    for r in rows {
        let date = r
            .get("date")
            .and_then(Value::as_str)
            .map(|s| s.split('T').next().unwrap_or(s).to_string())
            .unwrap_or_default();
        let atype = r
            .get("activityType")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        let amount = r
            .get("amount")
            .and_then(|v| {
                v.as_str()
                    .map(str::to_string)
                    .or_else(|| v.as_f64().map(|n| format!("{n}")))
            })
            .unwrap_or_default();
        let ccy = r
            .get("currency")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let comment = r
            .get("comment")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        println!(
            "{date:<10}  {atype:<12}  {amount:>14}  {ccy:<6}  {comment}",
            comment = if comment.chars().count() > 60 {
                let truncated: String = comment.chars().take(60).collect();
                format!("{truncated}…")
            } else {
                comment
            }
        );
    }
}

// ─────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn s(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn maps_required_columns() {
        let headers = s(&["date", "activityType", "amount", "currency"]);
        let rows = vec![s(&["2026-04-01", "WITHDRAWAL", "2000", "TWD"])];
        let acts = map_csv_to_activities(&headers, &rows, "acct-1").unwrap();
        assert_eq!(acts.len(), 1);
        let a = &acts[0];
        assert_eq!(a.date, "2026-04-01");
        assert_eq!(a.activity_type, "WITHDRAWAL");
        assert_eq!(a.amount.as_deref(), Some("2000"));
        assert_eq!(a.currency, "TWD");
        assert_eq!(a.account_id.as_deref(), Some("acct-1"));
    }

    #[test]
    fn header_case_and_underscore_insensitive() {
        let headers = s(&["Date", "ACTIVITY_TYPE", "Amount", "currency"]);
        let rows = vec![s(&["2026-04-01", "DEPOSIT", "100", "TWD"])];
        let acts = map_csv_to_activities(&headers, &rows, "acct-x").unwrap();
        assert_eq!(acts[0].activity_type, "DEPOSIT");
    }

    #[test]
    fn carries_optional_columns() {
        let headers = s(&[
            "date",
            "activityType",
            "amount",
            "currency",
            "symbol",
            "comment",
            "fee",
        ]);
        let rows = vec![s(&[
            "2026-04-01",
            "WITHDRAWAL",
            "2000",
            "TWD",
            "$CASH-TWD",
            "groceries",
            "15",
        ])];
        let acts = map_csv_to_activities(&headers, &rows, "acct-1").unwrap();
        let a = &acts[0];
        assert_eq!(a.symbol, "$CASH-TWD");
        assert_eq!(a.comment.as_deref(), Some("groceries"));
        assert_eq!(a.fee.as_deref(), Some("15"));
    }

    #[test]
    fn empty_cells_become_none() {
        let headers = s(&["date", "activityType", "amount", "currency", "fee"]);
        let rows = vec![s(&["2026-04-01", "DEPOSIT", "100", "TWD", ""])];
        let acts = map_csv_to_activities(&headers, &rows, "acct-1").unwrap();
        assert!(acts[0].fee.is_none());
    }

    #[test]
    fn strips_crlf_remnants() {
        let headers = s(&["date", "activityType", "amount", "currency"]);
        let rows = vec![s(&["2026-04-01", "DEPOSIT", "100", "TWD\r"])];
        let acts = map_csv_to_activities(&headers, &rows, "acct-1").unwrap();
        assert_eq!(acts[0].currency, "TWD");
    }

    #[test]
    fn unknown_columns_ignored() {
        let headers = s(&["date", "activityType", "amount", "currency", "ignored_col"]);
        let rows = vec![s(&["2026-04-01", "DEPOSIT", "100", "TWD", "x"])];
        let acts = map_csv_to_activities(&headers, &rows, "acct-1").unwrap();
        assert_eq!(acts.len(), 1);
        assert_eq!(acts[0].currency, "TWD");
    }

    #[test]
    fn missing_required_column_errors() {
        let headers = s(&["activityType", "amount", "currency"]);
        let rows: Vec<Vec<String>> = vec![];
        let err = map_csv_to_activities(&headers, &rows, "acct-1").unwrap_err();
        assert!(err.to_string().contains("'date'"));
    }

    #[test]
    fn empty_required_cell_errors() {
        let headers = s(&["date", "activityType", "amount", "currency"]);
        let rows = vec![s(&["", "DEPOSIT", "100", "TWD"])];
        let err = map_csv_to_activities(&headers, &rows, "acct-1").unwrap_err();
        assert!(err.to_string().contains("'date'"));
    }

    #[test]
    fn type_alias_accepted() {
        let headers = s(&["date", "type", "amount", "currency"]);
        let rows = vec![s(&["2026-04-01", "DEPOSIT", "100", "TWD"])];
        let acts = map_csv_to_activities(&headers, &rows, "acct-1").unwrap();
        assert_eq!(acts[0].activity_type, "DEPOSIT");
    }

    // ── parse_bulk_body ─────────────────────────────────────────────

    #[test]
    fn bulk_body_accepts_top_level_array() {
        let raw = r#"[
            {"accountId":"a","activityType":"DEPOSIT","activityDate":"2026-04-01","currency":"TWD","amount":"100"}
        ]"#;
        let body = parse_bulk_body(raw).unwrap();
        assert_eq!(body.creates.len(), 1);
        assert_eq!(body.updates.len(), 0);
        assert_eq!(body.delete_ids.len(), 0);
        assert_eq!(body.creates[0].activity_type, "DEPOSIT");
        assert_eq!(body.creates[0].amount.as_deref(), Some("100"));
    }

    #[test]
    fn bulk_body_accepts_full_mutation_object() {
        let raw = r#"{
            "creates": [
                {"accountId":"a","activityType":"DEPOSIT","activityDate":"2026-04-01","currency":"TWD"}
            ],
            "updates": [],
            "deleteIds": ["row-x", "row-y"]
        }"#;
        let body = parse_bulk_body(raw).unwrap();
        assert_eq!(body.creates.len(), 1);
        assert_eq!(body.delete_ids, vec!["row-x", "row-y"]);
    }

    #[test]
    fn bulk_body_empty_payload_errors() {
        let raw = r#"{"creates": [], "updates": [], "deleteIds": []}"#;
        let err = parse_bulk_body(raw).unwrap_err();
        assert!(err.to_string().contains("nothing to do"));
    }

    #[test]
    fn bulk_body_empty_array_errors() {
        let raw = "[]";
        let err = parse_bulk_body(raw).unwrap_err();
        assert!(err.to_string().contains("nothing to do"));
    }

    #[test]
    fn bulk_body_scalar_errors() {
        let err = parse_bulk_body("42").unwrap_err();
        assert!(err.to_string().contains("array or object"));
    }

    #[test]
    fn bulk_body_invalid_json_errors() {
        let err = parse_bulk_body("not json").unwrap_err();
        assert!(err.to_string().contains("invalid JSON"));
    }
}
