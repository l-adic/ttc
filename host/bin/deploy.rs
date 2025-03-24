use anyhow::{Ok, Result};
use clap::Parser;
use host::checkpoint::{Checkpoint, Checkpointer, ContractAddresses};
use host::cli::BaseConfig;
use host::contract::Artifacts;
use host::{
    contract::{
        nft::TestNFT,
        verifier::{MockVerifier, Verifier},
    },
    env::create_provider,
};
use risc0_steel::alloy::{
    network::Ethereum,
    primitives::U256,
    providers::Provider,
    signers::local::PrivateKeySigner,
    transports::http::{Client, Http},
};
use serde::Serialize;
use std::{path::Path, str::FromStr};
use tracing::info;
use url::Url;

pub mod contract {
    use risc0_steel::alloy::sol;

    sol!(
        #[sol(rpc, all_derives)]
        TopTradingCycle,
        "../contract/out/TopTradingCycle.sol/TopTradingCycle.json"
    );
}

#[derive(Clone, Parser, Serialize)]
pub struct DeployConfig {
    #[clap(flatten)]
    pub base: BaseConfig,

    #[arg(long, env = "NUM_ERC721", default_value_t = 3)]
    pub num_erc721: usize,

    #[arg(long, env = "MOCK_VERIFIER", default_value_t = false)]
    pub mock_verifier: bool,

    #[arg(long, env = "PHASE_DURATION", default_value_t = 0)]
    pub phase_duration: u64,
}

impl DeployConfig {
    pub fn node_url(&self) -> Result<Url, url::ParseError> {
        self.base.node_url()
    }
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
        *contract::TopTradingCycle::deploy(&provider, verifier, duration)
            .await?
            .address()
    };

    Ok(Artifacts { ttc, nft })
}

async fn deploy_contracts(config: DeployConfig) -> Result<ContractAddresses> {
    info!("{}", serde_json::to_string_pretty(&config).unwrap());

    let owner = PrivateKeySigner::from_str(config.base.owner_key.as_str())?;
    let node_url = config.node_url()?;
    let provider = create_provider(node_url.clone(), owner.clone());
    let Artifacts { ttc, nft } = deploy_for_test(
        config.num_erc721,
        config.phase_duration,
        provider.clone(),
        config.mock_verifier,
    )
    .await?;
    let checkpointer = {
        let checkpointer_root_dir = Path::new(&config.base.artifacts_dir);
        Checkpointer::new(checkpointer_root_dir, ttc)
    };
    // Get verifier address from TTC contract
    let ttc_contract = contract::TopTradingCycle::new(ttc, &provider);
    let verifier = ttc_contract.verifier().call().await?._0;
    let addresses = ContractAddresses { ttc, nft, verifier };
    checkpointer.save(Checkpoint::Deployed(addresses.clone()))?;
    Ok(addresses)
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = DeployConfig::parse();
    let addresses = deploy_contracts(config).await?;
    println!("{}", addresses.ttc);
    Ok(())
}
