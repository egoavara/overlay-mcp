use std::{collections::HashMap, str::FromStr};

use http::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};

use super::JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreDestination {
    pub method: String,
    pub header: HashMap<String, String>,
    pub query: HashMap<String, Vec<String>>,
    pub url: RestoreUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreSource {
    pub method: String,
    pub header: HashMap<String, String>,
    pub query: HashMap<String, Vec<String>>,
    pub url: RestoreUrl,
    pub context: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreUrl {
    pub scheme: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub path: String,
}

impl RestoreDestination {
    pub fn build_header_map(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (key, value) in &self.header {
            let keyname = HeaderName::from_str(key).unwrap();
            headers.insert(keyname, HeaderValue::from_str(value).unwrap());
        }
        headers
    }
}
