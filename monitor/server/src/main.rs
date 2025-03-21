use clap::Parser;
use jsonrpsee::{
    core::async_trait,
    server::Server,
    types::{ErrorObject, ErrorObjectOwned},
};
use monitor_api::{
    rpc::MonitorApiServer,
    types::{Proof, ProofStatus},
};
use monitor_server::{
    app_config::init_console_subscriber,
    db::{self, notify::JOB_CHANNEL, schema::JobStatus},
    ttc_contract, utils,
};
use risc0_steel::alloy::primitives::Address;
use std::{net::SocketAddr, sync::Arc};
use tracing::{debug, error, info};

mod app_env {
    use anyhow::Result;
    use clap::Parser;
    use monitor_server::{
        app_config,
        db::DB,
        monitor::{db::Database, events_manager::EventsManager},
        prover::remote::{self, Prover},
    };
    use serde::Serialize;
    use url::Url;

    #[derive(Parser, Serialize)]

    pub struct AppConfig {
        #[clap(flatten)]
        base_config: app_config::AppBaseConfig,

        #[arg(long, env = "JSON_RPC_PORT", default_value = "3030")]
        pub json_rpc_port: u16,

        #[arg(long, env = "PROVER_PROTOCOL", default_value = "http")]
        pub prover_protocol: String,

        /// Prover host
        #[arg(long, env = "PROVER_HOST", default_value = "localhost")]
        pub prover_host: String,

        /// Prover port (optional, not needed for Cloud Run)
        #[arg(long, env = "PROVER_PORT")]
        pub prover_port: Option<String>,

        #[arg(long, env = "PROVER_TIMEOUT", default_value = "120")]
        pub prover_timeout: u64,
    }

    impl AppConfig {
        pub fn prover_url(&self) -> Result<Url, url::ParseError> {
            let prover_url = match &self.prover_port {
                Some(port) => format!("{}://{}:{}", self.prover_protocol, self.prover_host, port),
                None => format!("{}://{}", self.prover_protocol, self.prover_host),
            };
            Url::parse(&prover_url)
        }
    }

    pub struct AppEnv {
        pub db: Database,
        pub node_url: Url,
        pub prover: remote::Prover,
        pub events_manager: EventsManager,
    }

    impl AppEnv {
        pub async fn new(app_config: AppConfig) -> Result<Self> {
            let db = {
                let db = DB::new(app_config.base_config.db_config()).await?;
                anyhow::Ok(Database::new(db.pool))
            }?
            .await;
            let node_url = app_config.base_config.node_url()?;
            let prover = {
                let prover_url = app_config.prover_url()?;
                let prover: remote::Prover =
                    Prover::new(node_url.clone(), prover_url, app_config.prover_timeout)?;
                anyhow::Ok(prover)
            }?;
            Ok(Self {
                db: db.clone(),
                node_url: node_url.clone(),
                prover: prover.clone(),
                events_manager: EventsManager::new(node_url, prover, db),
            })
        }
    }
}

use app_env::{AppConfig, AppEnv};

async fn listen_for_job_updates(env: Arc<AppEnv>) -> anyhow::Result<()> {
    let notifier =
        db::notify::PgNotifier::<Address>::new(&env.db.pool(), JOB_CHANNEL.clone()).await?;
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
        let proof_opt = self
            .app_env
            .db
            .get_proof_opt_by_address(address.as_slice())
            .await;
        match proof_opt {
            Ok(Some(proof)) => Ok(Proof {
                journal: proof.proof,
                seal: proof.seal,
            }),
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
        let status_opt = self.app_env.db.get_job_by_address(address.as_slice()).await;
        match status_opt {
            Ok(job) => {
                let status = match job.status {
                    JobStatus::Created => ProofStatus::Created,
                    JobStatus::InProgress => ProofStatus::InProgress,
                    JobStatus::Completed => ProofStatus::Completed,
                    JobStatus::Errored => ProofStatus::Errored(job.error.unwrap_or_default()),
                };

                Ok(status)
            }
            Err(err) => Err(ErrorObject::owned(-32001, err.to_string(), None::<()>)),
        }
    }

    async fn watch_contract(&self, address: Address) -> Result<(), ErrorObjectOwned> {
        let provider = utils::create_provider(self.app_env.node_url.clone());
        let ttc = ttc_contract::TopTradingCycle::new(address, provider);

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

    async fn get_image_id_contract(&self) -> Result<String, ErrorObjectOwned> {
        match self.app_env.prover.get_image_id_contract().await {
            Ok(contract) => Ok(contract),
            Err(err) => Err(ErrorObject::owned(-32001, err.to_string(), None::<()>)),
        }
    }

    async fn health_check(&self) -> Result<(), ErrorObjectOwned> {
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_console_subscriber();
    let cli = AppConfig::parse();
    debug!("{}", serde_json::to_string_pretty(&cli).unwrap());

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
