use super::types::Proof;
use jsonrpsee::{proc_macros::rpc, types::ErrorObjectOwned};
use risc0_steel::alloy::primitives::Address;

#[rpc(server, client)]
pub trait ProverApi {
    #[method(name = "prove")]
    async fn prove(&self, address: Address) -> Result<Proof, ErrorObjectOwned>;

    #[method(name = "proveAsync")]
    async fn prove_async(&self, address: Address) -> Result<(), ErrorObjectOwned>;

    #[method(name = "healthCheck")]
    async fn health_check(&self) -> Result<(), ErrorObjectOwned>;
}
