use anyhow::Result;
use nft::TestNFT;
use risc0_steel::alloy::{
    network::Ethereum,
    primitives::Address,
    providers::Provider,
    transports::http::{Client, Http},
};
use tracing::info;
use ttc::TopTradingCycle;
use verifier::{MockVerifier, Verifier};

pub mod nft;
pub mod ttc;
pub mod verifier;

pub struct Artifacts {
    pub ttc: Address,
    pub nft: Address,
}

pub async fn deploy(
    provider: impl Provider<Http<Client>, Ethereum>,
    dev_mode: bool,
) -> Result<Artifacts> {
    info!("Deploying NFT");
    let nft = *TestNFT::deploy(&provider).await?.address();
    info!("Deploying TTC");
    let ttc = {
        let verifier = if dev_mode {
            info!("Deploying MockVerifier");
            *MockVerifier::deploy(&provider).await?.address()
        } else {
            info!("Deploying Groth16Verifier");
            *Verifier::deploy(&provider).await?.address()
        };
        *TopTradingCycle::deploy(&provider, verifier)
            .await?
            .address()
    };
    Ok(Artifacts { ttc, nft })
}
