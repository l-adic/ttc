use clap::Parser;
use jsonrpsee::{
    core::async_trait,
    server::Server,
    types::{ErrorObject, ErrorObjectOwned},
};
use prover::rpc::ProverApiServer;
use prover_server::env::{init_console_subscriber, Config};
use risc0_steel::alloy::primitives::Address;
use std::net::SocketAddr;
use tracing::{error, info};
use url::Url;

pub struct ProverApiImpl {
    node_url: Url,
}

impl ProverApiImpl {
    fn new(node_url: Url) -> Self {
        Self { node_url }
    }
}

#[async_trait]
impl prover::rpc::ProverApiServer for ProverApiImpl {
    async fn prove(&self, address: Address) -> Result<prover::prover::Proof, ErrorObjectOwned> {
        let proof: anyhow::Result<prover::prover::Proof> = async {
            let provider = prover::prover::create_provider(self.node_url.clone());
            let ttc = prover::prover::ttc_contract::TopTradingCycle::new(address, provider);
            let phase = ttc.currentPhase().call().await?._0;
            if phase != 2 {
                anyhow::bail!("TTC contract is not in the trading phase");
            }
            info!("Starting prover for TTC contract at address: {:#}", address);
            let prover_cfg = prover::prover::ProverConfig {
                node_url: self.node_url.clone(),
                ttc: address,
            };
            let prover = prover::prover::Prover::new(&prover_cfg);
            let proof = prover.prove().await?;
            anyhow::Ok(proof)
        }
        .await;
        match proof {
            Ok(proof) => {
                info!("Prover completed successfully");
                Ok(proof)
            }
            Err(err) => {
                error!("Prover failed: {:#}", err);
                Err(ErrorObject::owned(-32001, err.to_string(), None::<()>))
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_console_subscriber();
    let cli = Config::parse();

    // Define the server address
    let addr = {
        let host = "127.0.0.1";
        let addr = format!("{}:{}", host, cli.json_rpc_port);
        addr.parse::<SocketAddr>()
    }?;

    // Create the JSON-RPC server
    let server = Server::builder().build(addr).await?;

    // Get the server's address
    let server_addr: SocketAddr = server.local_addr()?;
    info!("JSON-RPC server started at {}", server_addr);

    let api = {
        let node_url = Url::parse(&cli.node_url)?;
        anyhow::Ok(ProverApiImpl::new(node_url))
    }?;

    // Start the server with our API implementation
    let handle = server.start(api.into_rpc());

    // Keep the server running until Ctrl+C is pressed
    tokio::signal::ctrl_c().await?;

    // Stop the server
    handle.stop()?;
    info!("JSON-RPC server stopped");

    Ok(())
}
