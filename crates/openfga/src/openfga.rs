use std::{
    borrow::{Borrow, Cow},
    str::FromStr,
    sync::Arc,
};

use http::{HeaderMap, HeaderName, HeaderValue};
use url::Url;

use crate::{
    BatchCheckBody, BatchCheckElem, BatchCheckResponse, CheckBody, CheckPath, CheckResponse,
    ContextualTuple, Error, ListAllStoresQuery, ListAllStoresResponse, OpenfgaFailure, Tuple,
};

#[derive(Debug, Clone)]
pub struct Openfga {
    inner: Arc<OpenfgaRef>,
}

#[derive(Debug)]
pub struct OpenfgaRef {
    client: reqwest::Client,
    config: OpenfgaConfig,
    store_id: String,
}

#[derive(Debug)]
pub struct OpenfgaConfig {
    url: Url,
    store_name: String,
    headers: HeaderMap,
}

pub struct OpenfgaBuilder {
    result: Result<OpenfgaConfig, Error>,
}

impl OpenfgaBuilder {
    pub fn with_header<K: AsRef<str>, V: AsRef<str>>(self, key: K, value: V) -> Self {
        match self.result {
            Ok(mut config) => {
                match (
                    HeaderName::from_str(key.as_ref()),
                    HeaderValue::from_str(value.as_ref()),
                ) {
                    (Ok(header_name), Ok(header_value)) => {
                        config.headers.insert(header_name, header_value);
                        Self { result: Ok(config) }
                    }
                    (Err(err), _) => Self {
                        result: Err(Error::InvalidHeaderName(err)),
                    },
                    (_, Err(err)) => Self {
                        result: Err(Error::InvalidHeaderValue(err)),
                    },
                }
            }
            err @ Err(_) => Self { result: err },
        }
    }
    pub async fn connect(self) -> Result<Openfga, Error> {
        match self.result {
            Ok(config) => {
                let client = reqwest::Client::builder()
                    .default_headers(config.headers.clone())
                    .build()?;
                let store_name = config.store_name.clone();

                let resp = list_all_stores(
                    &config.url,
                    &client,
                    ListAllStoresQuery {
                        name: Some(store_name),
                        page_size: None,
                        continuation_token: None,
                    },
                )
                .await?;

                let store_id = resp.stores.first().ok_or(Error::StoreNotFound)?.id.clone();
                Ok(Openfga {
                    inner: Arc::new(OpenfgaRef {
                        client,
                        config,
                        store_id,
                    }),
                })
            }
            Err(err) => Err(err),
        }
    }
}
async fn list_all_stores(
    dest: &Url,
    client: &reqwest::Client,
    query: ListAllStoresQuery,
) -> Result<ListAllStoresResponse, Error> {
    let mut uri = dest.join("/stores")?;
    let query = serde_urlencoded::to_string(query)?;
    uri.set_query(Some(&query));

    let resp = client.get(uri).send().await?;
    Ok(resp.json::<ListAllStoresResponse>().await?)
}

impl Openfga {
    pub fn build<T: Into<String>>(url: Url, store_name: T) -> OpenfgaBuilder {
        OpenfgaBuilder {
            result: Ok(OpenfgaConfig {
                url,
                store_name: store_name.into(),
                headers: HeaderMap::new(),
            }),
        }
    }

    pub async fn list_all_stores<T: Into<Option<ListAllStoresQuery>>>(
        &self,
        query: T,
    ) -> Result<ListAllStoresResponse, Error> {
        let query: Option<ListAllStoresQuery> = query.into();
        let query = query.unwrap_or(ListAllStoresQuery {
            name: None,
            page_size: None,
            continuation_token: None,
        });
        list_all_stores(&self.inner.config.url, &self.inner.client, query).await
    }

    pub async fn check<S: Into<Option<CheckPath>>, T: Borrow<CheckBody>>(
        &self,
        path: S,
        body: T,
    ) -> Result<CheckResponse, Error> {
        let path: Option<CheckPath> = path.into();
        let body: &CheckBody = body.borrow();
        let path = path.unwrap_or_else(|| CheckPath {
            store_id: self.inner.store_id.clone(),
        });

        let checkurl = self
            .inner
            .config
            .url
            .join(&format!("/stores/{}/check", path.store_id))
            .expect("Failed to join url");
        tracing::info!(
            user = ?body.tuple_key.user,
            relation = ?body.tuple_key.relation,
            object = ?body.tuple_key.object,
            ctx = ?body.contextual_tuples.tuple_keys,
            "checking"
        );
        let resp = self.inner.client.post(checkurl).json(body).send().await?;
        if !resp.status().is_success() {
            let body: OpenfgaFailure = resp.json().await?;
            tracing::error!(error = ?body, "Failed to check");
            return Err(Error::CheckFailed(body));
        }
        let check_response: CheckResponse = resp.json().await?;
        Ok(check_response)
    }
    pub async fn batch_check<S: Into<Option<CheckPath>>, T: Borrow<BatchCheckBody>>(
        &self,
        path: S,
        body: T,
    ) -> Result<BatchCheckResponse, Error> {
        let path: Option<CheckPath> = path.into();
        let body: &BatchCheckBody = body.borrow();
        let path = path.unwrap_or_else(|| CheckPath {
            store_id: self.inner.store_id.clone(),
        });

        let checkurl = self
            .inner
            .config
            .url
            .join(&format!("/stores/{}/batch-check", path.store_id))
            .expect("Failed to join url");
        tracing::debug!("checking body: {:#?}", body);
        let resp = self.inner.client.post(checkurl).json(body).send().await?;
        if !resp.status().is_success() {
            let body: OpenfgaFailure = resp.json().await?;
            tracing::error!(error = ?body, "Failed to check");
            return Err(Error::CheckFailed(body));
        }
        let check_response: BatchCheckResponse = resp.json().await?;
        Ok(check_response)
    }

    pub async fn simple_check(&self, tuple: Tuple, context: Vec<Tuple>) -> Result<bool, Error> {
        let body = CheckBody {
            tuple_key: tuple,
            contextual_tuples: ContextualTuple {
                tuple_keys: context,
            },
            consistency: None,
        };
        let check_response = self.check(None, &body).await?;
        Ok(check_response.allowed)
    }

    pub async fn batch_simple_check<'a, I: IntoIterator<Item = Cow<'a, CheckBody>>>(
        &self,
        tuples: I,
    ) -> Result<Vec<bool>, Error> {
        let checks = tuples
            .into_iter()
            .map(|tuple| {
                let owned = tuple.into_owned();
                BatchCheckElem {
                    tuple_key: owned.tuple_key,
                    contextual_tuples: owned.contextual_tuples,
                    consistency: None,
                    correlation_id: uuid::Uuid::new_v4().to_string(),
                }
            })
            .collect::<Vec<_>>();
        let body = BatchCheckBody { checks };
        let mut resp = self.batch_check(None, &body).await?;
        let mut result = Vec::new();
        for c in &body.checks {
            let r = resp.result.remove(&c.correlation_id);
            result.push(r.is_some_and(|r| r.allowed));
        }
        Ok(result)
    }
}
