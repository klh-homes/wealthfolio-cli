//! Account schemas. Mirrors Wealthfolio's `Account` / `NewAccount` /
//! `AccountUpdate` from the server's OpenAPI spec exposed at
//! `/api/v1/openapi.json` (fetched 2026-05-15 against v3.4.0).

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

/// Body for `PUT /api/v1/accounts/{id}`. The server requires all
/// non-optional fields on this struct, so the typical client
/// flow is: fetch the full `Account`, mutate, send the result back.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountUpdate {
    pub id: String,
    pub name: String,
    pub account_type: String,
    pub currency: String,
    pub is_default: bool,
    pub is_active: bool,
    pub tracking_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_number: Option<String>,
}

impl From<&Account> for AccountUpdate {
    fn from(a: &Account) -> Self {
        Self {
            id: a.id.clone(),
            name: a.name.clone(),
            account_type: a.account_type.clone(),
            currency: a.currency.clone(),
            is_default: a.is_default,
            is_active: a.is_active,
            tracking_mode: a.tracking_mode.clone(),
            group: a.group.clone(),
            platform_id: a.platform_id.clone(),
            account_number: a.account_number.clone(),
        }
    }
}
