pub use crate::prover::types::Proof;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofStatus {
    Created,
    InProgress,
    Completed,
    Errored(String),
}
