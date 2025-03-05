use jsonrpsee::{proc_macros::rpc, types::ErrorObjectOwned};
use risc0_steel::alloy::primitives::Address;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    pub journal: Vec<u8>,
    pub seal: Vec<u8>,
}

#[rpc(server, client)]
pub trait ProverApi {
    #[method(name = "prove")]
    async fn prove(&self, address: Address) -> Result<Proof, ErrorObjectOwned>;
}
