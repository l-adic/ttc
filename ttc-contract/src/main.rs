use ethers::{
    middleware::SignerMiddleware,
    providers::{Http, Provider},
    signers::{LocalWallet, Signer},
};
use eyre::Result;
use std::str::FromStr;
use std::sync::Arc;
use ttc_contract::{nft::TestNFT, ttc::TopTradingCycle};

#[tokio::main]
async fn main() -> Result<()> {
    // Connect to local network (like anvil)
    let provider = Provider::<Http>::try_from("http://localhost:8545")?;
    let provider = Arc::new(provider);

    // Use default anvil private key
    let wallet = LocalWallet::from_str(
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
    )?;
    let client = Arc::new(SignerMiddleware::new(
        provider.clone(),
        wallet.with_chain_id(31337u64),
    ));

    // Deploy TestNFT
    println!("Deploying TestNFT...");
    let nft = TestNFT::deploy(client.clone(), ())?.send().await?;
    println!("TestNFT deployed to: {}", nft.address());

    // Deploy TTC with NFT address
    println!("Deploying TTC...");
    let ttc = TopTradingCycle::deploy(client.clone(), (nft.address(),))?
        .send()
        .await?;
    println!("TTC deployed to: {}", ttc.address());

    Ok(())
}
