use anyhow::Result;
use sqlx::{Executor, PgPool};
use tracing::info;

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

    // Create trigger function for notifications
    pool.execute(sqlx::query(
        r#"
        CREATE OR REPLACE FUNCTION notify_job_status_change()
        RETURNS TRIGGER AS $$
        BEGIN
            IF (NEW.status = 'completed' OR NEW.status = 'errored') AND 
               (OLD.status != 'completed' AND OLD.status != 'errored') THEN
                -- Convert BYTEA to hex string for the notification
                PERFORM pg_notify('job_channel', encode(NEW.address, 'hex'));
            END IF;
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql;
        "#,
    ))
    .await?;

    pool.execute(sqlx::query(
        r#"
        DO $$ 
        BEGIN
            -- Drop the trigger if it exists
            DROP TRIGGER IF EXISTS job_status_change_trigger ON jobs;
            
            -- Create the trigger
            CREATE TRIGGER job_status_change_trigger
            AFTER UPDATE OF status ON jobs
            FOR EACH ROW
            EXECUTE FUNCTION notify_job_status_change();
        END $$;
        "#,
    ))
    .await?;

    info!("Schema created successfully for database");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    monitor_server::app_env::init_console_subscriber();
    let db = monitor_server::app_env::DB::new_from_environment().await?;
    match create_schema(&db.pool).await {
        Ok(_) => {
            info!("Database schema setup completed successfully.");
            Ok(())
        }
        Err(e) => {
            tracing::error!("Error setting up database schema: {}", e);
            Err(e.into())
        }
    }
}
