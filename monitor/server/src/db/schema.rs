use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};

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
