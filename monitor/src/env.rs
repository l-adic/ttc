use anyhow::{Ok, Result};
use sqlx::postgres::{PgPool, PgPoolOptions};

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
}

pub struct Env {
    pub db: DB,
}

impl Env {
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
        let db = DB::new(db_config).await?;
        Ok(Self { db })
    }
}
