use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = monitor::env::DB::new_from_environment().await?.pool;
    let db_name = env::var("DB_NAME")?;

    // Check if database exists
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(&db_name)
            .fetch_one(&conn)
            .await?;

    if exists {
        println!("Database '{}' already exists.", db_name);
        return Ok(());
    }

    // Create database
    sqlx::query(&format!("CREATE DATABASE \"{}\"", db_name))
        .execute(&conn)
        .await?;

    println!("Database '{}' created successfully.", db_name);

    Ok(())
}
