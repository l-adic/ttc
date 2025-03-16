use super::types::{Proof, ProofStatus};
use jsonrpsee::{proc_macros::rpc, types::ErrorObjectOwned};
use risc0_steel::alloy::primitives::Address;

#[rpc(server, client)]
pub trait MonitorApi {
    #[method(name = "watchContract")]
    async fn watch_contract(&self, address: Address) -> Result<(), ErrorObjectOwned>;

    #[method(name = "getProof")]
    async fn get_proof(&self, address: Address) -> Result<Proof, ErrorObjectOwned>;

    #[method(name = "getProofStatus")]
    async fn get_proof_status(&self, address: Address) -> Result<ProofStatus, ErrorObjectOwned>;

    #[method(name = "healthCheck")]
    async fn health_check(&self) -> Result<(), ErrorObjectOwned>;
}
