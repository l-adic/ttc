use crate::db::schema::{Job, JobStatus, Proof};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

// Database management struct
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> PgPool {
        self.pool.clone()
    }

    pub async fn get_job_by_address(&self, address: &[u8]) -> Result<Job, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT 
                address, block_number, block_timestamp, 
                status, error, completed_at 
            FROM jobs 
            WHERE address = $1
        "#,
        )
        .bind(address)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_job_status(
        &self,
        address: &[u8],
        new_status: JobStatus,
        error: Option<String>,
        completed_at: Option<DateTime<Utc>>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE jobs 
            SET 
                status = $2, 
                error = $3, 
                completed_at = $4
            WHERE address = $1
        "#,
        )
        .bind(address)
        .bind(new_status)
        .bind(&error)
        .bind(completed_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Proof-specific methods
    pub async fn create_proof(&self, proof: &Proof) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO proofs (
                address, proof, seal
            ) VALUES (
                $1, $2, $3
            )
        "#,
        )
        .bind(&proof.address)
        .bind(&proof.proof)
        .bind(&proof.seal)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
