use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct CheckPath {
    pub store_id: String,
}

#[derive(Debug, Serialize)]
pub struct BatchCheckBody {
    pub checks: Vec<BatchCheckElem>,
}

#[derive(Debug, Serialize)]
pub struct BatchCheckElem {
    pub tuple_key: Tuple,
    pub contextual_tuples: ContextualTuple,
    pub consistency: Option<Consistency>,
    pub correlation_id: String,
}

#[derive(Debug, Deserialize)]
pub struct BatchCheckResponse {
    pub result: HashMap<String, BatchCheckElemResponse>,
}

#[derive(Debug, Deserialize)]
pub struct BatchCheckElemResponse {
    pub allowed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckBody {
    pub tuple_key: Tuple,
    pub contextual_tuples: ContextualTuple,
    pub consistency: Option<Consistency>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tuple {
    pub user: String,
    pub relation: String,
    pub object: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextualTuple {
    pub tuple_keys: Vec<Tuple>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum Consistency {
    #[serde(rename = "UNSPECIFIED")]
    Unspecified,
    #[serde(rename = "MINIMIZE_LATENCY")]
    MinimizeLatency,
    #[serde(rename = "HIGHER_CONSISTENCY")]
    HigherConsistency,
}

#[derive(Debug, Deserialize)]
pub struct CheckResponse {
    pub allowed: bool,

    #[allow(dead_code)]
    pub resolution: String,
}

#[derive(Debug, Serialize)]
pub struct ListAllStoresQuery {
    pub name: Option<String>,
    pub page_size: Option<u32>,
    pub continuation_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListAllStoresResponse {
    pub stores: Vec<Store>,
    pub continuation_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Store {
    pub name: String,
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct OpenfgaFailure {
    pub code: String,
    pub message: String,
}
