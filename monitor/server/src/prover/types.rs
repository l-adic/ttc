use risc0_steel::alloy::primitives::Address;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    pub journal: Vec<u8>,
    pub seal: Vec<u8>,
}

#[allow(async_fn_in_trait)]
pub trait ProverT {
    async fn prove(&self, address: Address) -> anyhow::Result<Proof>;
}

#[allow(async_fn_in_trait)]
pub trait AsyncProverT {
    async fn prove_async(&self, address: Address) -> anyhow::Result<()>;
}
