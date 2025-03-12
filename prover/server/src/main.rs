use clap::Parser;
use jsonrpsee::{
    core::async_trait,
    server::Server,
    types::{ErrorObject, ErrorObjectOwned},
};
use monitor_common::db::{self, JobStatus};
use prover_common::rpc::{Proof, ProverApiServer};
use prover_server::prover::ProverT;
use prover_server::{
    app_env::{init_console_subscriber, AppConfig, AppEnv},
    prover,
};
use risc0_steel::alloy::primitives::Address;
use sqlx::types::chrono;
use std::net::SocketAddr;
use tracing::{error, info};

#[derive(Clone)]
pub struct ProverApiImpl {
    app_env: AppEnv,
}

impl ProverApiImpl {
    fn new(app_env: AppEnv) -> Self {
        Self { app_env }
    }

    async fn assert_in_trade_phase(&self, address: Address) -> Result<(), ErrorObjectOwned> {
        let provider = prover::create_provider(self.app_env.node_url.clone());
        let ttc = prover::ttc_contract::TopTradingCycle::new(address, provider);
        let e_phase = ttc.currentPhase().call().await;
        match e_phase {
            Ok(phase) => {
                if phase._0 != 2 {
                    let err_str = format!(
                        "TTC contract is not in the trading phase, current phase is {}",
                        phase._0
                    );
                    tracing::error!(err_str);
                    Err(ErrorObject::owned(-32001, err_str, None::<()>))
                } else {
                    Ok(())
                }
            }
            Err(e) => Err(ErrorObject::owned(-32001, e.to_string(), None::<()>)),
        }
    }

    async fn prove_impl(&self, address: Address) -> anyhow::Result<Proof> {
        info!("Starting prover for TTC contract at address: {:#}", address);
        let proof = self.app_env.prover.prove(address).await;
        match proof {
            Ok(proof) => {
                info!("Prover successful, writing to DB");
                let db_proof = db::Proof {
                    address: address.as_slice().to_vec(),
                    proof: proof.journal.clone(),
                    seal: proof.seal.clone(),
                };
                self.app_env.db.create_proof(&db_proof).await?;
                let now = chrono::Utc::now();
                self.app_env
                    .db
                    .update_job_status(address.as_slice(), JobStatus::Completed, None, Some(now))
                    .await?;
                Ok(proof)
            }
            Err(err) => {
                let err_str = err.to_string();
                error!("Prover errored with message {}", err_str);
                let now = chrono::Utc::now();
                self.app_env
                    .db
                    .update_job_status(
                        address.as_slice(),
                        JobStatus::Errored,
                        Some(err_str),
                        Some(now),
                    )
                    .await?;
                Err(err)
            }
        }
    }
}

#[async_trait]
impl ProverApiServer for ProverApiImpl {
    async fn prove(&self, address: Address) -> Result<Proof, ErrorObjectOwned> {
        let res = self.prove_impl(address).await;
        match res {
            Ok(proof) => Ok(proof),
            Err(err) => Err(ErrorObject::owned(-32001, err.to_string(), None::<()>)),
        }
    }

    async fn prove_async(&self, address: Address) -> Result<(), ErrorObjectOwned> {
        self.assert_in_trade_phase(address).await?;
        let api = self.clone();
        tokio::spawn(async move {
            api.prove_impl(address).await?;
            anyhow::Ok(())
        });
        Ok(())
    }

    async fn health_check(&self) -> Result<(), ErrorObjectOwned> {
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_console_subscriber();
    let cli = AppConfig::parse();
    // Define the server address
    let addr = {
        let host = "0.0.0.0";
        let addr = format!("{}:{}", host, cli.json_rpc_port);
        addr.parse::<SocketAddr>()
    }?;

    let app_env = AppEnv::new(cli).await?;

    // Create the JSON-RPC server
    let server = Server::builder().build(addr).await?;

    // Get the server's address
    let server_addr: SocketAddr = server.local_addr()?;
    info!("JSON-RPC server started at {}", server_addr);

    let api = ProverApiImpl::new(app_env);

    // Start the server with our API implementation
    let handle = server.start(api.into_rpc());

    // Keep the server running until Ctrl+C is pressed
    tokio::signal::ctrl_c().await?;

    // Stop the server
    handle.stop()?;
    info!("JSON-RPC server stopped");

    Ok(())
}
