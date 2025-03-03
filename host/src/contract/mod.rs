use anyhow::{Ok, Result};
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
    pub nft: Vec<Address>,
}

pub async fn deploy(
    provider: impl Provider<Http<Client>, Ethereum> + Clone,
    dev_mode: bool,
    n_721: usize,
) -> Result<Artifacts> {
    info!("Deploying NFT");

    // Deploy NFTs sequentially to avoid nonce conflicts
    let mut nft = Vec::with_capacity(n_721);
    for _ in 0..n_721 {
        let contract = TestNFT::deploy(&provider).await?;
        let address = *contract.address();
        info!("Deployed NFT at {}", address);
        nft.push(address);
    }

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
