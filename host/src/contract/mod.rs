use risc0_steel::alloy::primitives::Address;

pub mod nft;
pub mod ttc;
pub mod verifier;

pub struct Artifacts {
    pub ttc: Address,
    pub nft: Vec<Address>,
}
