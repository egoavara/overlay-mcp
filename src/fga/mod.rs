mod schema;

use std::{collections::HashMap, str::FromStr};

use anyhow::{Context, Ok, Result};
use http::{HeaderMap, HeaderName, HeaderValue};
use jsonpath_rust::JsonPath;
use serde::Deserialize;
use serde_json::json;
use url::Url;

#[derive(Debug, Clone)]
pub struct Fga {
    client: reqwest::Client,
    url: Url,
    store_id: String,
    authorization_model_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CheckResponse {
    allowed: bool,
    resolution: String,
}

impl Fga {
    pub async fn init(
        url: Url,
        store_name: String,
        headers: &HashMap<String, String>,
    ) -> Result<Self> {
        let mut default_headers = HeaderMap::new();
        for (key, value) in headers {
            default_headers.insert(HeaderName::from_str(key)?, HeaderValue::from_str(value)?);
        }

        let client = reqwest::Client::builder()
            .default_headers(default_headers)
            .build()?;
        let store_id = Self::upsert_store(&client, &url, &store_name).await?;
        tracing::info!("Store ID: {}", store_id);
        Self::upsert_authorization_model(&client, &url, &store_id).await?;
        let fga = Self {
            client,
            url,
            store_id,
            authorization_model_id: None,
        };
        Ok(fga)
    }

    async fn upsert_store(client: &reqwest::Client, url: &Url, store_name: &str) -> Result<String> {
        let mut founduri = url.join("/stores").context("Failed to join url")?;
        founduri.query_pairs_mut().append_pair("name", store_name);

        let resp = client
            .get(founduri)
            .send()
            .await
            .context("Failed to send request")?
            .json::<serde_json::Value>()
            .await
            .context("Failed to parse response body")?;
        let found = resp.query("$.stores[*].id")?;
        match found.as_slice() {
            [x] => Ok(x.as_str().expect("store_id must be string").to_string()),
            [x, _, ..] => {
                tracing::error!("Multiple stores found, please check your FGA configuration");
                Err(anyhow::anyhow!(
                    "Multiple stores found, please check your FGA configuration"
                ))
            }
            [] => {
                tracing::info!("Store not found, creating new store");
                let inserturi = url.join("/stores").context("Failed to join url")?;
                let resp = client
                    .post(inserturi)
                    .json(&json!({
                        "name": store_name
                    }))
                    .send()
                    .await
                    .context("Failed to send request")?
                    .json::<serde_json::Value>()
                    .await
                    .context("Failed to parse response body")?;
                Ok(resp.query("$.id")?.as_slice()[0]
                    .as_str()
                    .expect("store_id must be string")
                    .to_string())
            }
        }
    }

    async fn upsert_authorization_model(
        client: &reqwest::Client,
        url: &Url,
        store_id: &str,
    ) -> Result<String> {
        let founduri = url
            .join(&format!("/stores/{}/authorization-models", store_id))
            .context("Failed to join url")?;
        let resp = client
            .get(founduri)
            .send()
            .await
            .context("Failed to send request")?
            .json::<serde_json::Value>()
            .await
            .context("Failed to parse response body")?;
        tracing::info!("Authorization model: {:#?}", resp);
        let found = resp.query("$.authorization_models[*].id")?;
        match found.as_slice() {
            [x, ..] => {
                tracing::info!("Authorization model found, using existing authorization model");
                tracing::info!("Authorization model ID: {:#?}", x.to_string());
                // TODO: 기존 권한 모델이 유효한지 확인
                Ok(x.to_string())
            }
            [] => {
                tracing::info!("Authorization model not found, creating new authorization model");
                let inserturi = url
                    .join(&format!("/stores/{}/authorization-models", store_id))
                    .context("Failed to join url")?;
                let resp = client
                    .post(inserturi)
                    .json(&schema::schema())
                    .send()
                    .await?
                    .json::<serde_json::Value>()
                    .await?;
                tracing::info!("Authorization model created: {:#?}", resp);
                Ok(resp.query("$.authorization_model_id")?.as_slice()[0]
                    .as_str()
                    .expect("authorization_model_id must be string")
                    .to_string())
            }
        }
    }
    pub async fn check(
        &self,
        tuple: (String, String, String),
        context: Vec<(String, String, String)>,
    ) -> Result<bool> {
        let checkurl = self
            .url
            .join(&format!("/stores/{}/check", self.store_id))
            .expect("Failed to join url");
        let contextual = context
            .iter()
            .map(|(user, relation, object)| {
                json!({
                    "user": user,
                    "relation": relation,
                    "object": object
                })
            })
            .collect::<Vec<_>>();

        let body = json!({
            "tuple_key": {
                "user": tuple.0,
                "relation": tuple.1,
                "object": tuple.2
            },
            "contextual_tuples":{
                "tuple_keys": contextual,
            },
        });
        tracing::info!("body: {:#?}", body);
        let resp = self
            .client
            .post(checkurl)
            .json(&body)
            .send()
            .await?
            .json::<CheckResponse>()
            .await?;

        Ok(resp.allowed)
    }
}
