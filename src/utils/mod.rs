use std::{collections::HashMap, str::FromStr};

use anyhow::{Context, Result};
use axum::body::Body;
use http::{request::Parts, HeaderName, HeaderValue, Method, Request};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum HttpComponent {
    #[serde(rename = "query")]
    Query { name: String },
    #[serde(rename = "header")]
    Header { name: String },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Passthrough {
    pub from: HttpComponent,

    /// If not specified, the value will be passed as is.
    #[serde(default)]
    pub to: Option<HttpComponent>,
}

pub struct PassthroughState {
    pub(crate) src: Parts,
    pub(crate) src_query: HashMap<String, String>,
    pub(crate) dst_query: HashMap<String, String>,
    pub(crate) dst_headers: Vec<(HeaderName, HeaderValue)>,
}

impl PassthroughState {
    pub fn new(src: Parts) -> Self {
        let src_query = if let Some(query) = src.uri.query() {
            form_urlencoded::parse(query.as_bytes())
                .into_owned()
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::new()
        };
        Self {
            src,
            src_query,
            dst_query: HashMap::new(),
            dst_headers: Vec::new(),
        }
    }

    pub fn end<T>(self, method: Method, mut url: Url, body: T) -> Result<Request<T>> {
        url.query_pairs_mut().extend_pairs(self.dst_query.iter());

        let builder = http::request::Builder::new()
            .uri(url.as_str())
            .method(method)
            .version(self.src.version);

        builder.body(body).context("http error")
    }

    pub fn empty_end(self, method: Method, url: Url) -> Result<Request<Body>> {
        self.end(method, url, Body::empty())
    }

    pub fn passing(mut self, passthrough: &Passthrough) -> Result<Self> {
        let value = match &passthrough.from {
            HttpComponent::Query { name } => match self.src_query.get(name) {
                Some(value) => PassthroughValue::String(value.to_string()),
                None => PassthroughValue::None,
            },
            HttpComponent::Header { name } => match self.src.headers.get(name) {
                Some(value) => match value.to_str() {
                    Ok(value) => PassthroughValue::String(value.to_string()),
                    Err(_) => PassthroughValue::Bytes(value.as_bytes()),
                },
                None => PassthroughValue::None,
            },
        };
        let to = passthrough.to.as_ref().unwrap_or(&passthrough.from);
        match to {
            HttpComponent::Query { name } => match value {
                PassthroughValue::String(value) => {
                    self.dst_query.insert(name.clone(), value);
                }
                PassthroughValue::Bytes(_) => {
                    tracing::warn!("Bytes value is not supported for query");
                }
                PassthroughValue::None => {}
            },
            HttpComponent::Header { name } => {
                let header_name = HeaderName::from_str(name)?;
                match value {
                    PassthroughValue::String(value) => match HeaderValue::from_str(&value) {
                        Ok(value) => {
                            self.dst_headers.push((header_name, value));
                        }
                        Err(_) => {
                            tracing::warn!(input = ?value,  "Failed to parse header value");
                        }
                    },
                    PassthroughValue::Bytes(value) => match HeaderValue::from_bytes(value) {
                        Ok(value) => {
                            self.dst_headers.push((header_name, value));
                        }
                        Err(_) => {
                            tracing::warn!(input = ?value,  "Failed to parse header value");
                        }
                    },
                    PassthroughValue::None => {}
                }
            }
        }
        Ok(self)
    }
}

pub enum PassthroughValue<'a> {
    String(String),
    Bytes(&'a [u8]),
    None,
}
