//! Activity-related schemas. Only what we touch for the `import` flow —
//! we hand off CSV to the server, so we only need the response shape
//! (parsed CSV preview + import outcome).
//!
//! Reference: `apps/server/src/api/activities.rs` + `docs/activities/activity-types.md`
//! (Wealthfolio v3.4.0).

use serde::{Deserialize, Serialize};

/// Response from `POST /api/v1/activities/import/parse` — the server
/// inspects the CSV and tells us what it sees before any rows are written.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportParseResponse {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    #[serde(default)]
    pub detected_config: serde_json::Value,
    #[serde(default)]
    pub errors: Vec<serde_json::Value>,
    #[serde(default)]
    pub row_count: u64,
}

/// Response from `POST /api/v1/activities/import` — actual write outcome.
/// Loose shape (upstream is undocumented), surfaced as JSON for inspection.
pub type ImportResponse = serde_json::Value;
