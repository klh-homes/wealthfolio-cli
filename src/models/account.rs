//! Account schemas. Mirrors Wealthfolio's `Account` / `NewAccount` from the
//! server's OpenAPI spec exposed at `/api/v1/openapi.json` (fetched 2026-05-15
//! against v3.4.0).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub name: String,
    pub account_type: String,
    pub currency: String,
    pub is_default: bool,
    pub is_active: bool,
    pub is_archived: bool,
    pub tracking_mode: String,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub platform_id: Option<String>,
    #[serde(default)]
    pub account_number: Option<String>,
    #[serde(default)]
    pub meta: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub provider_account_id: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAccount {
    pub name: String,
    pub account_type: String,
    pub currency: String,
    pub is_default: bool,
    pub is_active: bool,
    /// "HOLDINGS" or "TRANSACTIONS"
    pub tracking_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_number: Option<String>,
}
