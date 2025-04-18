use clap::Parser;
use figment::providers::Serialized;
use figment::value::{Dict, Map as FigmentMap, Value as FigmentValue};
use figment::{Metadata, Profile, Provider};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use url::Url;

#[derive(Parser, Debug, Clone, Deserialize, Serialize)]
#[command(version, about, long_about = None)]
pub struct Command {
    #[arg(short, long = "config", env = "OVERLAY_MCP_CONFIG_FILE")]
    pub configfile: Option<PathBuf>,

    #[arg(short,long = "log-filter", env = "OVERLAY_MCP_LOG_FILTER", default_value_t = String::from("warn"))]
    pub log_filter: String,

    #[arg(long = "prometheus", env = "OVERLAY_MCP_PROMETHEUS", default_value_t = false)]
    pub prometheus: bool,

    #[arg(long = "health-check", env = "OVERLAY_MCP_HEALTH_CHECK", default_value_t = true)]
    pub health_check: bool,

    #[arg(long = "addr", env = "OVERLAY_MCP_SERVER_ADDR", default_value_t = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9090))]
    pub addr: SocketAddr,

    #[arg(long = "hostname", env = "OVERLAY_MCP_SERVER_HOSTNAME")]
    pub hostname: Option<Url>,

    #[arg(long = "upstream", env = "OVERLAY_MCP_SERVER_UPSTREAM")]
    pub upstream: Option<Url>,

    #[arg(long = "otel-endpoint", env = "OVERLAY_MCP_OTEL_ENDPOINT")]
    pub endpoint: Option<String>,

    #[arg(long = "oidc-issuer", env = "OVERLAY_MCP_OIDC_ISSUER")]
    pub issuer: Option<String>,

    #[arg(long = "oidc-client-id", env = "OVERLAY_MCP_OIDC_CLIENT_ID")]
    pub client_id: Option<String>,

    #[arg(long = "oidc-client-secret", env = "OVERLAY_MCP_OIDC_CLIENT_SECRET")]
    pub client_secret: Option<String>,

    #[arg(long = "oidc-scopes", env="OVERLAY_MCP_OIDC_SCOPE", value_delimiter = ',', num_args = 1..)]
    pub scopes: Option<Vec<String>>,
}

// null 또는 빈 객체를 재귀적으로 제거하는 함수
fn clean_json(value: JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let cleaned_map: JsonMap<String, JsonValue> = map
                .into_iter()
                .filter_map(|(k, v)| {
                    let cleaned_v = clean_json(v);
                    if cleaned_v.is_null()
                        || (cleaned_v.is_object() && cleaned_v.as_object().unwrap().is_empty())
                    {
                        None // null 이거나 빈 객체면 제거
                    } else {
                        Some((k, cleaned_v))
                    }
                })
                .collect();
            JsonValue::Object(cleaned_map)
        }
        JsonValue::Array(arr) => {
            // 배열 내의 각 요소에 대해 재귀적으로 정리
            let cleaned_arr = arr.into_iter().map(clean_json).collect();
            JsonValue::Array(cleaned_arr)
        }
        // 다른 타입은 그대로 반환
        _ => value,
    }
}

impl Provider for Command {
    fn metadata(&self) -> Metadata {
        Metadata::named("command-line arguments")
    }

    fn data(&self) -> figment::Result<FigmentMap<Profile, Dict>> {
        let mut data = json!({
            "application": {
                "log_filter": self.log_filter,
                "prometheus": self.prometheus,
                "health_check": self.health_check,
            },
            "server": {
                "addr": self.addr,
                "hostname": self.hostname,
                "upstream": self.upstream,
            },
            "idp": {
                "issuer": self.issuer,
                "client_id": self.client_id,
                "client_secret": self.client_secret,
                "scopes": self.scopes,
            },
            "otel": {
                "endpoint": self.endpoint,
            }
        });

        // null 또는 빈 객체 필드 제거
        data = clean_json(data);

        if let Some(obj) = data.as_object_mut() {
            obj.retain(|_, v| !(v.is_object() && v.as_object().unwrap().is_empty()));
        }

        tracing::debug!(
            "Cleaned config data from args: {}",
            serde_json::to_string(&data)
                .unwrap_or_else(|e| format!("Error serializing config: {}", e))
        );

        // Figment가 요구하는 타입으로 변환 시도
        let figment_value: FigmentValue = serde_json::from_value(data).map_err(|e| {
            figment::Error::from(format!(
                "Failed to convert cleaned JSON to Figment Value: {}",
                e
            ))
        })?;

        // 최종적으로 Figment의 Map<Profile, Dict> 형태로 변환
        Serialized::from(figment_value, Profile::Default).data()
    }
}
