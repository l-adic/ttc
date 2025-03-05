use jsonrpsee::{proc_macros::rpc, types::ErrorObjectOwned};
use risc0_steel::alloy::primitives::Address;

use crate::prover;

#[rpc(server, client)]
pub trait ProverApi {
    #[method(name = "prove")]
    async fn prove(&self, address: Address) -> Result<prover::Proof, ErrorObjectOwned>;
}
