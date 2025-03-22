use anyhow::{Ok, Result};
use risc0_steel::alloy::{
    network::Ethereum,
    primitives::{Address, U256},
    providers::Provider,
    transports::http::{Client, Http},
};
use tracing::info;

use crate::contract::{
    nft::TestNFT,
    ttc::TopTradingCycle,
    verifier::{MockVerifier, Verifier},
};

pub struct Artifacts {
    pub ttc: Address,
    pub nft: Vec<Address>,
}

pub async fn deploy_for_test(
    num_erc721: usize,
    phase_duration: u64,
    provider: impl Provider<Http<Client>, Ethereum> + Clone,
    dev_mode: bool,
) -> Result<Artifacts> {
    info!("Deploying NFT");

    // Deploy NFTs sequentially to avoid nonce conflicts
    let mut nft = Vec::with_capacity(num_erc721);
    for _ in 0..num_erc721 {
        let contract = TestNFT::deploy(&provider).await?;
        let address = *contract.address();
        info!("Deployed NFT at {:#}", address);
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
        let duration = U256::from(phase_duration);
        *TopTradingCycle::deploy(&provider, verifier, duration)
            .await?
            .address()
    };

    Ok(Artifacts { ttc, nft })
}
