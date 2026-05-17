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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_account() -> Account {
        Account {
            id: "uuid-1".into(),
            name: "LINE Bank".into(),
            account_type: "SAVINGS".into(),
            currency: "TWD".into(),
            is_default: false,
            is_active: true,
            is_archived: false,
            tracking_mode: "HOLDINGS".into(),
            group: Some("Personal".into()),
            platform_id: None,
            account_number: Some("****1651".into()),
            meta: None,
            provider: None,
            provider_account_id: None,
            created_at: Some("2026-05-15T00:00:00".into()),
            updated_at: Some("2026-05-15T00:00:00".into()),
        }
    }

    #[test]
    fn account_update_from_account_preserves_required_fields() {
        let a = sample_account();
        let u = AccountUpdate::from(&a);
        assert_eq!(u.id, "uuid-1");
        assert_eq!(u.name, "LINE Bank");
        assert_eq!(u.account_type, "SAVINGS");
        assert_eq!(u.currency, "TWD");
        assert_eq!(u.tracking_mode, "HOLDINGS");
        assert_eq!(u.group.as_deref(), Some("Personal"));
        assert_eq!(u.account_number.as_deref(), Some("****1651"));
        assert!(!u.is_default);
        assert!(u.is_active);
    }

    #[test]
    fn account_update_serializes_camel_case_and_skips_none() {
        let mut u = AccountUpdate::from(&sample_account());
        u.tracking_mode = "TRANSACTIONS".into();
        u.platform_id = None;
        let v: serde_json::Value = serde_json::to_value(&u).unwrap();
        let obj = v.as_object().unwrap();
        assert_eq!(obj.get("trackingMode").unwrap(), "TRANSACTIONS");
        assert_eq!(obj.get("accountType").unwrap(), "SAVINGS");
        // None fields are dropped
        assert!(!obj.contains_key("platformId"));
        // group is Some, present
        assert_eq!(obj.get("group").unwrap(), "Personal");
    }

    #[test]
    fn account_update_round_trip_does_not_drop_fields() {
        // Update with no changes should produce a body identical to
        // round-trip of the original account's mutable fields.
        let a = sample_account();
        let u = AccountUpdate::from(&a);
        let v = serde_json::to_value(&u).unwrap();
        let obj = v.as_object().unwrap();
        assert_eq!(obj.get("id").unwrap(), "uuid-1");
        assert_eq!(obj.get("name").unwrap(), "LINE Bank");
        assert_eq!(obj.get("currency").unwrap(), "TWD");
        assert_eq!(obj.get("isActive").unwrap(), true);
        assert_eq!(obj.get("isDefault").unwrap(), false);
    }
}
