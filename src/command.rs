use clap::{arg, Args, Parser, Subcommand};
use serde_json::{json, Map, Value};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use url::Url;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Subcommands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Subcommands {
    Run(SubcommandRun),
}

#[derive(Args, Debug, Clone)]
pub struct SubcommandRun {
    #[arg(short, long = "config", env = "OVERLAY_MCP_CONFIG_FILE")]
    pub configfile: Option<PathBuf>,

    #[arg(
        short,
        long = "log-filter",
        env = "OVERLAY_MCP_LOG_FILTER",
        default_value_t = String::from("warn")
    )]
    pub log_filter: String,

    #[arg(
        long = "prometheus",
        env = "OVERLAY_MCP_PROMETHEUS",
        default_value_t = false
    )]
    pub prometheus: bool,

    #[arg(
        long = "health-check",
        env = "OVERLAY_MCP_HEALTH_CHECK",
        default_value_t = true
    )]
    pub health_check: bool,

    #[arg(long = "raft-id", env = "OVERLAY_MCP_RAFT_ID")]
    pub raft_id: Option<u64>,

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
fn clean_json(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let cleaned_map: Map<String, Value> = map
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
            Value::Object(cleaned_map)
        }
        Value::Array(arr) => {
            // 배열 내의 각 요소에 대해 재귀적으로 정리
            let cleaned_arr = arr.into_iter().map(clean_json).collect();
            Value::Array(cleaned_arr)
        }
        // 다른 타입은 그대로 반환
        _ => value,
    }
}

impl SubcommandRun {
    pub fn figment_default() -> figment::providers::Serialized<figment::value::Value> {
        
        let figment_value: figment::value::Value = serde_json::from_value(json!({
            "application": {
                "log_filter": "warn",
                "prometheus": false,
                "health_check": true,
            },
            "server": {
                "addr": "0.0.0.0:9090"
            }
        })).unwrap();
        // 최종적으로 Figment의 Map<Profile, Dict> 형태로 변환
        figment::providers::Serialized::from(figment_value, figment::Profile::Default)
    }
    pub fn figment_merge(&self) -> figment::providers::Serialized<figment::value::Value> {
        let mut cluster = serde_json::Map::new();
        if let Some(raft_id) = self.raft_id {
            cluster.insert("type".to_string(), json!("raft"));
            cluster.insert("id".to_string(), json!(raft_id));
        }

        let mut result = clean_json(json!({
            "application": {
            },
            "upstream": {
                "urls": self.upstream,
            },
            "server": {
                "addr": self.addr,
                "hostname": self.hostname,
                "cluster": cluster,
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
        }));
        if !self.prometheus {
            result["application"]["prometheus"] = json!(true);
        }
        if self.health_check {
            result["application"]["health_check"] = json!(false);
        }
        if self.log_filter != "warn" {
            result["application"]["log_filter"] = json!(self.log_filter);
        }
        if self.addr != SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9090) {
            result["server"]["addr"] = json!(self.addr);
        }
        
        let figment_value: figment::value::Value = serde_json::from_value(result).unwrap();
        // 최종적으로 Figment의 Map<Profile, Dict> 형태로 변환
        figment::providers::Serialized::from(figment_value, figment::Profile::Default)
    }
}
