use crate::db::schema::{Job, Proof};
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

    // Job-specific methods
    pub async fn create_job(&self, job: &Job) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO jobs (
                address, block_number, block_timestamp, 
                status, error, completed_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6
            )
        "#,
        )
        .bind(&job.address)
        .bind(job.block_number)
        .bind(job.block_timestamp)
        .bind(job.status)
        .bind(&job.error)
        .bind(job.completed_at)
        .execute(&self.pool)
        .await?;

        Ok(())
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

    pub async fn get_proof_by_address(&self, address: &[u8]) -> Result<Proof, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT address, proof, seal 
            FROM proofs 
            WHERE address = $1
        "#,
        )
        .bind(address)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_proof_opt_by_address(
        &self,
        address: &[u8],
    ) -> Result<Option<Proof>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT address, proof, seal 
            FROM proofs 
            WHERE address = $1
        "#,
        )
        .bind(address)
        .fetch_optional(&self.pool)
        .await
    }
}
