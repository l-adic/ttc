use anyhow::Result;
use sqlx::{Executor, PgPool};

async fn create_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Create ENUM type
    pool.execute(sqlx::query(
        r#"
        DO $$ BEGIN
            IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'job_status') THEN
                CREATE TYPE job_status AS ENUM (
                    'created',
                    'in_progress',
                    'completed',
                    'errored'
                );
            END IF;
        END $$;
    "#,
    ))
    .await?;

    // Create Jobs table
    pool.execute(sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS jobs (
            address BYTEA PRIMARY KEY,
            block_number BIGINT NOT NULL,
            block_timestamp TIMESTAMPTZ NOT NULL,
            status job_status NOT NULL,
            error TEXT,
            completed_at TIMESTAMPTZ
        )
    "#,
    ))
    .await?;

    // Create indexes
    pool.execute(sqlx::query(
        r#"
        DO $$ BEGIN
            CREATE INDEX IF NOT EXISTS idx_jobs_block_number ON jobs (block_number);
            CREATE INDEX IF NOT EXISTS idx_jobs_block_timestamp ON jobs (block_timestamp);
            CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs (status);
        END $$;
    "#,
    ))
    .await?;

    // Create Proofs table
    pool.execute(sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS proofs (
            address BYTEA PRIMARY KEY,
            proof BYTEA NOT NULL,
            seal BYTEA NOT NULL
        )
    "#,
    ))
    .await?;

    println!("Schema created successfully for database");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let db = monitor::env::DB::new_from_environment().await?;

    match create_schema(&db.pool).await {
        Ok(_) => {
            println!("Database schema setup completed successfully.");
            Ok(())
        }
        Err(e) => {
            eprintln!("Error setting up database schema: {}", e);
            Err(e.into())
        }
    }
}
