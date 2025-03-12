use alloy::primitives::Address;
use jsonrpsee::{proc_macros::rpc, types::ErrorObjectOwned};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    pub journal: Vec<u8>,
    pub seal: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofStatus {
    Created,
    InProgress,
    Completed,
    Errored(String),
}

#[rpc(server, client)]
pub trait MonitorApi {
    #[method(name = "watchContract")]
    async fn watch_contract(&self, address: Address) -> Result<(), ErrorObjectOwned>;

    #[method(name = "getProof")]
    async fn get_proof(&self, address: Address) -> Result<Proof, ErrorObjectOwned>;

    #[method(name = "getProofStatus")]
    async fn get_proof_status(&self, address: Address) -> Result<ProofStatus, ErrorObjectOwned>;
}
