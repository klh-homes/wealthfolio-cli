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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn de_str_or_num_accepts_string() {
        #[derive(Deserialize)]
        struct W {
            #[serde(default, deserialize_with = "de_str_or_num")]
            v: Option<String>,
        }
        let w: W = serde_json::from_value(json!({"v": "2000.5"})).unwrap();
        assert_eq!(w.v.as_deref(), Some("2000.5"));
    }

    #[test]
    fn de_str_or_num_accepts_number() {
        #[derive(Deserialize)]
        struct W {
            #[serde(default, deserialize_with = "de_str_or_num")]
            v: Option<String>,
        }
        let w: W = serde_json::from_value(json!({"v": 2000.5})).unwrap();
        assert_eq!(w.v.as_deref(), Some("2000.5"));

        let w: W = serde_json::from_value(json!({"v": 42})).unwrap();
        assert_eq!(w.v.as_deref(), Some("42"));
    }

    #[test]
    fn de_str_or_num_accepts_bool_as_stringified() {
        #[derive(Deserialize)]
        struct W {
            #[serde(default, deserialize_with = "de_str_or_num")]
            v: Option<String>,
        }
        // Bool falls through the catch-all branch; we accept and stringify.
        let w: W = serde_json::from_value(json!({"v": true})).unwrap();
        assert_eq!(w.v.as_deref(), Some("true"));
    }

    #[test]
    fn de_str_or_num_accepts_null_and_missing() {
        #[derive(Deserialize)]
        struct W {
            #[serde(default, deserialize_with = "de_str_or_num")]
            v: Option<String>,
        }
        let w: W = serde_json::from_value(json!({"v": null})).unwrap();
        assert!(w.v.is_none());
        let w: W = serde_json::from_value(json!({})).unwrap();
        assert!(w.v.is_none());
    }

    #[test]
    fn activity_import_drops_none_fields_on_serialize() {
        let a = ActivityImport {
            date: "2026-04-01".into(),
            symbol: String::new(),
            activity_type: "WITHDRAWAL".into(),
            currency: "TWD".into(),
            amount: Some("2000".into()),
            comment: Some("transfer".into()),
            account_id: Some("acct-1".into()),
            is_valid: true,
            ..Default::default()
        };
        let v: Value = serde_json::to_value(&a).unwrap();
        let obj = v.as_object().unwrap();
        // Required fields present
        assert!(obj.contains_key("date"));
        assert!(obj.contains_key("activityType"));
        assert!(obj.contains_key("currency"));
        // Required-empty-but-always-serialized: symbol stays as ""
        assert_eq!(obj.get("symbol"), Some(&json!("")));
        // None fields skipped
        assert!(!obj.contains_key("quantity"));
        assert!(!obj.contains_key("unitPrice"));
        assert!(!obj.contains_key("fee"));
        assert!(!obj.contains_key("subtype"));
        assert!(!obj.contains_key("fxRate"));
        assert!(!obj.contains_key("id"));
        // Some fields populated
        assert_eq!(obj.get("amount"), Some(&json!("2000")));
        assert_eq!(obj.get("comment"), Some(&json!("transfer")));
        // Server-populated fields never serialized
        assert!(!obj.contains_key("errors"));
        assert!(!obj.contains_key("warnings"));
        assert!(!obj.contains_key("duplicateOfId"));
    }

    #[test]
    fn activity_import_deserializes_server_response_shape() {
        // Mirrors what /import/check actually returns: amounts as numbers,
        // server-populated id/errors/warnings/duplicateOfId.
        let body = json!({
            "id": "7a49d623",
            "date": "2026-04-01",
            "symbol": "",
            "activityType": "WITHDRAWAL",
            "currency": "TWD",
            "amount": 2000.0,
            "fee": null,
            "comment": "transfer",
            "accountId": "acct-1",
            "isDraft": false,
            "isValid": true,
            "errors": null,
            "warnings": {"_duplicate": ["Duplicate"]},
            "duplicateOfId": "abc-123"
        });
        let a: ActivityImport = serde_json::from_value(body).unwrap();
        // serde_json preserves float repr; "2000.0" not "2000"
        assert_eq!(a.amount.as_deref(), Some("2000.0"));
        assert_eq!(a.duplicate_of_id.as_deref(), Some("abc-123"));
        assert!(a.warnings.is_some());
    }

    #[test]
    fn new_activity_cash_serialization() {
        let n = NewActivity {
            account_id: "acct-1".into(),
            activity_type: "WITHDRAWAL".into(),
            activity_date: "2026-04-01".into(),
            amount: Some("2000".into()),
            currency: "TWD".into(),
            notes: Some("[main] transfer".into()),
            source_system: Some("LINE_BANK".into()),
            source_record_id: Some("linebank-1651-2026-04-01-126357".into()),
            status: Some("POSTED".into()),
            ..Default::default()
        };
        let v: Value = serde_json::to_value(&n).unwrap();
        let obj = v.as_object().unwrap();
        // No asset field on cash (asset: None is skipped)
        assert!(!obj.contains_key("asset"));
        assert!(!obj.contains_key("quantity"));
        assert!(!obj.contains_key("unitPrice"));
        assert!(!obj.contains_key("subtype"));
        // Tagging fields preserved
        assert_eq!(obj.get("sourceSystem"), Some(&json!("LINE_BANK")));
        assert_eq!(
            obj.get("sourceRecordId"),
            Some(&json!("linebank-1651-2026-04-01-126357"))
        );
        // Notes (NOT comment) is the canonical NewActivity field name
        assert_eq!(obj.get("notes"), Some(&json!("[main] transfer")));
        assert!(!obj.contains_key("comment"));
    }

    #[test]
    fn new_activity_accepts_comment_alias_on_deserialize() {
        // Server / client tools sometimes call it "comment"; we accept
        // both on the way in.
        let v = json!({
            "accountId": "acct-1",
            "activityType": "DEPOSIT",
            "activityDate": "2026-04-01",
            "currency": "TWD",
            "comment": "via alias"
        });
        let n: NewActivity = serde_json::from_value(v).unwrap();
        assert_eq!(n.notes.as_deref(), Some("via alias"));
    }

    #[test]
    fn search_request_omits_none_filters() {
        let r = ActivitySearchRequest {
            page: 0,
            page_size: 50,
            account_id_filter: Some("acct-1".into()),
            ..Default::default()
        };
        let v: Value = serde_json::to_value(&r).unwrap();
        let obj = v.as_object().unwrap();
        assert_eq!(obj.get("page"), Some(&json!(0)));
        assert_eq!(obj.get("pageSize"), Some(&json!(50)));
        assert_eq!(obj.get("accountIdFilter"), Some(&json!("acct-1")));
        assert!(!obj.contains_key("dateFrom"));
        assert!(!obj.contains_key("dateTo"));
        assert!(!obj.contains_key("activityTypeFilter"));
    }
}
