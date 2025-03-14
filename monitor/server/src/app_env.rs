use crate::{db::Database, events_manager::EventsManager, prover::remote::Prover};
use alloy::{
    network::Ethereum,
    providers::{Provider, ProviderBuilder},
    transports::http::{Client, Http},
};
use anyhow::{Ok, Result};
use clap::Parser;
use serde::Serialize;
use sqlx::postgres::{PgPool, PgPoolOptions};
use time::macros::format_description;
use tracing_subscriber::{
    fmt::{format::FmtSpan, time::UtcTime},
    EnvFilter,
};
use url::Url;

#[derive(Clone)]
pub struct DBConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub dbname: String,
}

impl DBConfig {
    pub fn connection_string(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.dbname
        )
    }
}

#[derive(Clone)]
pub struct DB {
    pub pool: PgPool,
}

impl DB {
    pub async fn new(config: DBConfig) -> Result<Self> {
        let connection_string = config.connection_string();
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&connection_string)
            .await?;
        Ok(Self { pool })
    }

    pub async fn new_from_environment() -> Result<Self> {
        let db_config = {
            let host = std::env::var("DB_HOST")?;
            let port = std::env::var("DB_PORT")?.parse()?;
            let user = std::env::var("DB_USER")?;
            let password = std::env::var("DB_PASSWORD")?;
            let dbname = std::env::var("DB_NAME")?;
            Ok(DBConfig {
                host,
                port,
                user,
                password,
                dbname,
            })
        }?;
        Self::new(db_config).await
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

pub fn create_provider(node_url: Url) -> impl Provider<Http<Client>, Ethereum> + Clone {
    ProviderBuilder::new().on_http(node_url)
}

pub struct AppEnv {
    pub db: Database,
    pub prover: Prover,
    pub node_url: Url,
    pub events_manager: EventsManager,
}

impl AppEnv {
    pub async fn new(app_config: AppConfig) -> Result<Self> {
        let db = {
            let db = DB::new(app_config.db_config()).await?;
            Ok(Database::new(db.pool))
        }?
        .await;
        let node_url = app_config.node_url()?;
        let prover = Prover::new(
            node_url.clone(),
            app_config.prover_url()?,
            db.clone(),
            app_config.prover_timeout_secs,
        )?;
        let events_manager = EventsManager::new(node_url.clone(), prover.clone(), db.clone());
        Ok(Self {
            db,
            prover,
            node_url,
            events_manager,
        })
    }
}

#[derive(Parser, Debug, Clone, Serialize)]
#[command(author, version, about, long_about = None)]
pub struct AppConfig {
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

    /// Prover Protocol
    #[arg(long, env = "PROVER_PROTOCOL", default_value = "http")]
    pub prover_protocol: String,

    /// Prover host
    #[arg(long, env = "PROVER_HOST", default_value = "localhost")]
    pub prover_host: String,

    /// Prover port (optional, not needed for Cloud Run)
    #[arg(long, env = "PROVER_PORT")]
    pub prover_port: Option<String>,

    #[arg(long, env = "JSON_RPC_PORT", default_value = "3030")]
    pub json_rpc_port: u16,

    #[arg(long, env = "PROVER_TIMEOUT_SECS", default_value = "300")]
    pub prover_timeout_secs: u64,
}

impl AppConfig {
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

    /// Get the prover URL
    pub fn prover_url(&self) -> Result<Url, url::ParseError> {
        let prover_url = match &self.prover_port {
            Some(port) => format!("{}://{}:{}", self.prover_protocol, self.prover_host, port),
            None => format!("{}://{}", self.prover_protocol, self.prover_host),
        };
        Url::parse(&prover_url)
    }
}
