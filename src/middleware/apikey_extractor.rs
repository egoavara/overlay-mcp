use std::{collections::HashMap, ops::Deref, sync::Arc};

use anyhow::Result;
use axum::extract::{FromRef, FromRequestParts};
use http::{request::Parts, StatusCode};

use crate::utils::HttpComponent;

#[derive(Clone)]
pub struct ApikeyExtractorState(Arc<ApikeyExtractorStateInner>);

impl ApikeyExtractorState {
    pub async fn load(extractor: Vec<HttpComponent>) -> Result<Self> {
        Ok(Self(Arc::new(ApikeyExtractorStateInner {
            extractor,
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
    pub(crate) extractor: Vec<HttpComponent>,
}

pub struct OptApikey(pub Option<(String, HttpComponent)>);

impl<S> FromRequestParts<S> for OptApikey
where
    ApikeyExtractorState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ApikeyExtractorState(state) = ApikeyExtractorState::from_ref(state);

        let mut result: Option<(String, HttpComponent)> = None;
        let mut lazy_query: Option<HashMap<String, String>> = None;
        for extractor in &state.extractor {
            match extractor {
                HttpComponent::Query { name } => {
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
                HttpComponent::Header { name } => {
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
