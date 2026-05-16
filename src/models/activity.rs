//! Activity-related schemas. Two write paths to the server:
//!
//! * `/api/v1/activities/import/{parse,check,import}` — CSV-style flow.
//!   The CLI uploads the file to `/parse` (multipart), maps the parsed
//!   rows into `ActivityImport` objects, then POSTs JSON to `/check` /
//!   `/import`. Server uses `(account_id, activity_type, date,
//!   asset_id, qty, price, amount, currency, comment)` as the
//!   idempotency key — the `id` / `source_record_id` on `ActivityImport`
//!   are NOT honored here (hardcoded `None` in the upstream service).
//!
//! * `/api/v1/activities/{,bulk}` — typed flow using `NewActivity`.
//!   Honors `source_record_id` in the idempotency hash, supports
//!   `source_system` tagging, custom `idempotency_key`, etc. Duplicates
//!   cause an atomic HTTP 400 (the whole batch is rejected), so
//!   pipelines should pre-filter via `/api/v1/activities/search`.
//!
//! Reference: `crates/core/src/activities/activities_model.rs` in
//! the Wealthfolio v3.4.0 source tree.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Accept either a JSON string or number, surface as `Option<String>`.
/// Server-side decimal fields come back as numbers, but on submit we
/// pass strings (safer for high-precision amounts).
fn de_str_or_num<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Option::<Value>::deserialize(deserializer)?;
    Ok(match v {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(other) => Some(other.to_string()),
    })
}

// ─────────────────────────────────────────────────────────────────────
// /api/v1/activities/import/parse — multipart CSV in, parsed rows out.
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportParseResponse {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    #[serde(default)]
    pub detected_config: Value,
    #[serde(default)]
    pub errors: Vec<Value>,
    #[serde(default)]
    pub row_count: u64,
}

// ─────────────────────────────────────────────────────────────────────
// /api/v1/activities/import/{check,import} — JSON in, JSON out.
// ─────────────────────────────────────────────────────────────────────

/// Wire-format row sent to `/check` and `/import`. Only the fields we
/// actually populate are serialized — the rest are server-side
/// validation noise that's ignored on the way in and surfaced (if
/// present) on the way out.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityImport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub date: String,
    /// Empty string or `$CASH-XXX` for cash activities — server clears
    /// it via `is_cash_symbol` / `is_garbage_symbol`. Required (`String`,
    /// not `Option`) per the upstream schema.
    #[serde(default)]
    pub symbol: String,
    pub activity_type: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de_str_or_num"
    )]
    pub quantity: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de_str_or_num"
    )]
    pub unit_price: Option<String>,
    pub currency: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de_str_or_num"
    )]
    pub fee: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de_str_or_num"
    )]
    pub amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de_str_or_num"
    )]
    pub fx_rate: Option<String>,
    #[serde(default)]
    pub is_draft: bool,
    #[serde(default = "default_true")]
    pub is_valid: bool,

    // Fields populated by the server on validation — we deserialize so
    // callers can inspect them, but they're never sent.
    #[serde(default, skip_serializing)]
    pub errors: Option<Value>,
    #[serde(default, skip_serializing)]
    pub warnings: Option<Value>,
    #[allow(dead_code)]
    #[serde(default, skip_serializing)]
    pub duplicate_of_id: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportCheckBody<'a> {
    pub activities: &'a [ActivityImport],
}

/// Response from `/import` — keep this loose since `summary` shape is
/// undocumented and may evolve.
pub type ImportResponse = Value;

// ─────────────────────────────────────────────────────────────────────
// /api/v1/activities + /api/v1/activities/bulk — NewActivity flow.
// ─────────────────────────────────────────────────────────────────────

/// Wire-format for creating a single activity or for the `creates` slot
/// of a bulk mutation. Mirrors `NewActivity` in the server crate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewActivity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub account_id: String,
    /// Asset resolution input. For cash activities, leave `None`. For
    /// asset trades, send `{ "symbol": "AAPL" }` or richer shapes the
    /// server understands.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<Value>,
    pub activity_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    pub activity_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_price: Option<String>,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    /// `POSTED`, `PENDING`, `DRAFT`, or `VOID`. Defaults server-side to
    /// `POSTED`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, alias = "comment", skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fx_rate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needs_review: Option<bool>,
    /// Originating system tag. Examples: `LINE_BANK`, `CSV`, `MANUAL`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_system: Option<String>,
    /// Provider-side stable record id. Feeds the idempotency hash —
    /// this is the recommended way to give the server a deterministic
    /// dedup key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_record_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_group_id: Option<String>,
    /// Pre-computed idempotency hash. When set, server uses it as-is
    /// instead of computing from content. Useful for intentional
    /// duplicates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityBulkMutationRequest {
    pub creates: Vec<NewActivity>,
    pub updates: Vec<Value>,
    pub delete_ids: Vec<String>,
}

/// Response shape from `/activities/bulk`. Loose — the server may add
/// fields over time.
pub type ActivityBulkMutationResult = Value;

// ─────────────────────────────────────────────────────────────────────
// /api/v1/activities/search — pagination + filtering.
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySearchRequest {
    /// 0-indexed page number.
    pub page: i64,
    pub page_size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id_filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_type_filter: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_to: Option<String>,
}

/// Loose response — `meta.totalRowCount` plus a `data: [Activity]`
/// array.
pub type ActivitySearchResponse = Value;
