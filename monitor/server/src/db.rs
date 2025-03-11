use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Type};

// Custom type for JobStatus to map to PostgreSQL ENUM
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "job_status", rename_all = "snake_case")]
pub enum JobStatus {
    Created,
    InProgress,
    Completed,
    Errored,
}

// Job table representation
#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Job {
    pub address: Vec<u8>,
    pub block_number: i64,
    pub block_timestamp: DateTime<Utc>,
    pub status: JobStatus,
    pub error: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
}

// Proof table representation
#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Proof {
    pub address: Vec<u8>,
    pub proof: Vec<u8>,
    pub seal: Vec<u8>,
}

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
}
