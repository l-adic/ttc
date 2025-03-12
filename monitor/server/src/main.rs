use alloy::primitives::Address;
use clap::Parser;
use jsonrpsee::{
    core::async_trait,
    server::Server,
    types::{ErrorObject, ErrorObjectOwned},
};
use monitor_common::{
    pg_notify::JOB_CHANNEL,
    rpc::{MonitorApiServer, Proof, ProofStatus},
};
use monitor_server::app_env::{init_console_subscriber, AppConfig, AppEnv};
use monitor_server::pg_notify::PgNotifier;
use monitor_server::prover::ProverT;
use std::{net::SocketAddr, sync::Arc};
use tracing::{debug, error, info};

async fn listen_for_job_updates(env: Arc<AppEnv>) -> anyhow::Result<()> {
    let notifier = PgNotifier::new(&env.db.pool(), JOB_CHANNEL.clone()).await?;
    let mut subs = notifier.subscribe();

    // Clone the Arc to share ownership
    let env_clone = env.clone();

    tokio::spawn(async move {
        while let Some(job) = subs.recv().await {
            if let Err(e) = env_clone.events_manager.cancel_monitoring(job).await {
                tracing::error!("Failed to cancel monitoring: {}", e);
            }
        }
    });

    Ok(())
}

struct ProverApiImpl {
    app_env: Arc<AppEnv>,
}

#[async_trait]
impl MonitorApiServer for ProverApiImpl {
    async fn get_proof(&self, address: Address) -> Result<Proof, ErrorObjectOwned> {
        debug!("Getting proof for address: {:#}", address);
        let proof_opt = self.app_env.prover.get_proof(address).await;
        match proof_opt {
            Ok(Some(proof)) => Ok(proof),
            Ok(None) => Err(ErrorObject::owned(
                -32001,
                "Proof not found".to_string(),
                None::<()>,
            )),
            Err(err) => Err(ErrorObject::owned(-32001, err.to_string(), None::<()>)),
        }
    }

    async fn get_proof_status(&self, address: Address) -> Result<ProofStatus, ErrorObjectOwned> {
        debug!("Getting proof status for address: {:#}", address);
        let status_opt = self.app_env.prover.get_proof_status(address).await;
        match status_opt {
            Ok(Some(status)) => Ok(status),
            Ok(None) => Err(ErrorObject::owned(
                -32001,
                "Proof not found".to_string(),
                None::<()>,
            )),
            Err(err) => Err(ErrorObject::owned(-32001, err.to_string(), None::<()>)),
        }
    }

    async fn watch_contract(&self, address: Address) -> Result<(), ErrorObjectOwned> {
        let provider = monitor_server::utils::create_provider(self.app_env.node_url.clone());
        let ttc = monitor_server::ttc_contract::TopTradingCycle::new(address, provider);

        // Get the phase and handle errors explicitly
        let phase = match ttc.currentPhase().call().await {
            Ok(phase) => phase,
            Err(err) => {
                error!("Failed to get current phase: {:#}", err);
                return Err(ErrorObject::owned(
                    -32001,
                    format!("Failed to get current phase: {}", err),
                    None::<()>,
                ));
            }
        };

        if phase._0 >= 2 {
            return Err(ErrorObject::owned(
                -32001,
                format!(
                    "TTC contract has already entered the trading phase, current phase is {}",
                    phase._0
                ),
                None::<()>,
            ));
        }

        // Get the block number with explicit error handling
        let from_block = match ttc.tradeInitiatedAtBlock().call().await {
            Ok(block) => block._0.try_into().unwrap(),
            Err(err) => {
                error!("Failed to get trade initiated block: {:#}", err);
                return Err(ErrorObject::owned(
                    -32001,
                    format!("Failed to get trade initiated block: {}", err),
                    None::<()>,
                ));
            }
        };

        debug!(
            "Watching TTC contract {} from block number {}",
            address, from_block
        );

        // Now monitor the contract
        match self
            .app_env
            .events_manager
            .monitor_trade_phase(address, from_block)
            .await
        {
            Ok(()) => Ok(()),
            Err(err) => {
                error!("Failed to watch contract: {:#}", err);
                Err(ErrorObject::owned(-32001, err.to_string(), None::<()>))
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_console_subscriber();
    let cli = AppConfig::parse();

    // Define the server address
    let addr = {
        let host = "0.0.0.0";
        let addr = format!("{}:{}", host, cli.json_rpc_port.clone());
        addr.parse::<SocketAddr>()
    }?;

    let app_env = {
        let e = AppEnv::new(cli).await?;
        Arc::new(e)
    };
    listen_for_job_updates(app_env.clone()).await?;

    // Create the JSON-RPC server
    let server = Server::builder().build(addr).await?;

    // Get the server's address
    let server_addr: SocketAddr = server.local_addr()?;
    info!("JSON-RPC server started at {}", server_addr);

    let api = ProverApiImpl { app_env };

    // Start the server with our API implementation
    let handle = server.start(api.into_rpc());

    // Keep the server running until Ctrl+C is pressed
    tokio::signal::ctrl_c().await?;

    // Stop the server
    handle.stop()?;
    info!("JSON-RPC server stopped");

    Ok(())
}
