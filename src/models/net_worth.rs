//! Net-worth schemas. Hand-written from `apps/server/src/api/net_worth.rs`
//! since the upstream OpenAPI spec only documents accounts CRUD.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetWorth {
    pub date: String,
    pub assets: NetWorthBucket,
    pub liabilities: NetWorthBucket,
    pub net_worth: f64,
    pub currency: String,
    #[serde(default)]
    pub oldest_valuation_date: Option<String>,
    #[serde(default)]
    pub stale_assets: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetWorthBucket {
    pub total: f64,
    #[serde(default)]
    pub breakdown: Vec<NetWorthBreakdownEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetWorthBreakdownEntry {
    /// Keep loose: server may add fields per release.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
