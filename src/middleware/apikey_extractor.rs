use std::{collections::HashMap, ops::Deref, str::FromStr, sync::Arc};

use anyhow::Result;
use axum::extract::{FromRef, FromRequestParts};
use http::{request::Parts, uri, StatusCode, Uri};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::config::Config;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ApikeyExtractor {
    #[serde(rename = "query")]
    Query { name: String },
    #[serde(rename = "header")]
    Header { name: String },
}

impl ApikeyExtractor {
    pub fn destruct(&self, mut parts: Parts) -> Parts {
        match self {
            ApikeyExtractor::Query { name } => {
                let mut url = Url::from_str(&parts.uri.to_string()).unwrap();
                if let Some(query) = url.query() {
                    let mut query_map = form_urlencoded::parse(query.as_bytes())
                        .into_owned()
                        .collect::<HashMap<_, _>>();
                    query_map.remove(name);
                    let query = form_urlencoded::Serializer::new(String::new())
                        .extend_pairs(query_map.into_iter())
                        .finish();
                    url.set_query(Some(&query));
                }
                parts.uri = Uri::from_str(&url.to_string()).unwrap();
            }
            ApikeyExtractor::Header { name } => {
                parts.headers.remove(name);
            }
        }
        parts
    }
}

#[derive(Clone)]
pub struct ApikeyExtractorState(Arc<ApikeyExtractorStateInner>);

impl ApikeyExtractorState {
    pub async fn load(extractor: Vec<ApikeyExtractor>) -> Result<Self> {
        Ok(Self(Arc::new(ApikeyExtractorStateInner {
            extractor: extractor,
        })))
    }
}

impl Deref for ApikeyExtractorState {
    type Target = ApikeyExtractorStateInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct ApikeyExtractorStateInner {
    pub(crate) extractor: Vec<ApikeyExtractor>,
}

pub struct OptApikey(pub Option<(String, ApikeyExtractor)>);

impl<S> FromRequestParts<S> for OptApikey
where
    ApikeyExtractorState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ApikeyExtractorState(state) = ApikeyExtractorState::from_ref(state);

        let mut result: Option<(String, ApikeyExtractor)> = None;
        let mut lazy_query: Option<HashMap<String, String>> = None;
        for extractor in &state.extractor {
            match extractor {
                ApikeyExtractor::Query { name } => {
                    let query = if let Some(query) = &lazy_query {
                        query
                    } else {
                        if let Some(query) = parts.uri.query() {
                            lazy_query.replace(
                                form_urlencoded::parse(query.as_bytes())
                                    .into_owned()
                                    .collect(),
                            );
                        } else {
                            lazy_query.replace(HashMap::new());
                        }
                        lazy_query.as_ref().unwrap()
                    };
                    if let Some(value) = query.get(name) {
                        return Ok(OptApikey(Some((value.to_string(), extractor.clone()))));
                    }
                }
                ApikeyExtractor::Header { name } => {
                    if let Some(value) = parts.headers.get(name) {
                        match value.to_str() {
                            Ok(value) => {
                                result.replace((value.to_string(), extractor.clone()));
                            }
                            Err(err) => {
                                tracing::warn!(
                                    value = ?value,
                                    error = ?err,
                                    "unexpected header value"
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(OptApikey(result))
    }
}
