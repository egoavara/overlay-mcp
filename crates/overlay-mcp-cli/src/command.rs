use clap::{arg, Args, Parser, Subcommand};
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

    #[arg(long = "raft-index", env = "OVERLAY_MCP_RAFT_INDEX")]
    pub raft_index: Option<u64>,

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
