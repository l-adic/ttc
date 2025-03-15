use std::env;

use monitor::server::{app_config, db};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    app_config::init_console_subscriber();
    let conn = db::DB::new_from_environment().await?.pool;
    let db_name = env::var("DB_CREATE_NAME")?;
    info!("Creating database '{}'", db_name);

    // Check if database exists
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(&db_name)
            .fetch_one(&conn)
            .await?;

    if exists {
        info!("Database '{}' already exists.", db_name);
        return Ok(());
    }

    // Create database
    sqlx::query(&format!("CREATE DATABASE \"{}\"", db_name))
        .execute(&conn)
        .await?;

    info!("Database '{}' created successfully.", db_name);

    Ok(())
}
