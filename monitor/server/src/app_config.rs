use crate::db::DBConfig;
use anyhow::Result;
use clap::Parser;
use serde::Serialize;
use time::macros::format_description;
use tracing_subscriber::{
    fmt::{format::FmtSpan, time::UtcTime},
    EnvFilter,
};
use url::Url;

#[derive(Parser, Debug, Clone, Serialize)]
#[command(author, version, about, long_about = None)]
pub struct AppBaseConfig {
    /// Database host
    #[arg(long, env = "DB_HOST", default_value = "localhost")]
    pub db_host: String,

    /// Database port
    #[arg(long, env = "DB_PORT", default_value = "5432")]
    pub db_port: u16,

    /// Database user
    #[arg(long, env = "DB_USER", default_value = "postgres")]
    pub db_user: String,

    /// Database password
    #[arg(long, env = "DB_PASSWORD", env)]
    pub db_password: String,

    /// Database name
    #[arg(long, env = "DB_NAME", default_value = "app")]
    pub db_name: String,

    /// Node host
    #[arg(long, env = "NODE_HOST", default_value = "localhost")]
    pub node_host: String,

    /// Node port
    #[arg(long, env = "NODE_PORT", default_value = "8545")]
    pub node_port: String,
}

impl AppBaseConfig {
    /// Get the database configuration
    pub fn db_config(&self) -> DBConfig {
        DBConfig {
            host: self.db_host.clone(),
            port: self.db_port,
            user: self.db_user.clone(),
            password: self.db_password.clone(),
            dbname: self.db_name.clone(),
        }
    }

    /// Get the node URL
    pub fn node_url(&self) -> Result<Url, url::ParseError> {
        let node_url = format!("http://{}:{}", self.node_host, self.node_port);
        Url::parse(&node_url)
    }
}

pub fn init_console_subscriber() {
    let timer = UtcTime::new(format_description!(
        "[year]-[month]-[day]T[hour repr:24]:[minute]:[second].[subsecond digits:3]Z"
    ));
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_span_events(FmtSpan::CLOSE)
        .with_timer(timer)
        .with_target(true)
        .with_thread_ids(false)
        .with_line_number(false)
        .with_file(false)
        .with_level(true)
        .with_ansi(true)
        .with_writer(std::io::stdout)
        .init();
}
