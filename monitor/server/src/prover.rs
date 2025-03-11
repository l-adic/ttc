use alloy::primitives::Address;
use monitor_common::rpc::{Proof, ProofStatus};

#[allow(async_fn_in_trait)]
pub trait ProverT {
    async fn prove(&self, address: Address) -> anyhow::Result<Proof>;
    async fn prove_async(&self, address: Address) -> anyhow::Result<()>;
    async fn get_proof(&self, address: Address) -> anyhow::Result<Option<Proof>>;
    async fn get_proof_status(&self, address: Address) -> anyhow::Result<Option<ProofStatus>>;
}

pub mod remote {
    use super::{Proof, ProofStatus, ProverT};
    use crate::{
        db::{Database, JobStatus},
        ttc_contract, utils,
    };
    use alloy::{
        network::Ethereum,
        primitives::Address,
        providers::Provider,
        transports::http::{Client, Http},
    };
    use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
    use prover_common::rpc::ProverApiClient;
    use url::Url;

    async fn assert_in_trade_phase(
        provider: impl Provider<Http<Client>, Ethereum>,
        address: Address,
    ) -> anyhow::Result<()> {
        let ttc = ttc_contract::TopTradingCycle::new(address, provider);
        let phase = ttc.currentPhase().call().await?._0;
        if phase != 2 {
            anyhow::bail!(
                "TTC contract is not in the trading phase, current phase is {}",
                phase
            );
        }
        Ok(())
    }

    #[derive(Clone)]
    pub struct Prover {
        node_url: Url,
        client: HttpClient,
        db: Database,
    }

    impl Prover {
        pub fn new(node_url: Url, prover_url: Url, db: Database) -> anyhow::Result<Self> {
            let client = HttpClientBuilder::default().build(prover_url)?;
            Ok(Self {
                node_url,
                client,
                db,
            })
        }
    }

    impl ProverT for Prover {
        async fn prove(&self, address: Address) -> anyhow::Result<Proof> {
            let provider = utils::create_provider(self.node_url.clone());
            assert_in_trade_phase(provider, address).await?;
            let p = ProverApiClient::prove(&self.client, address)
                .await
                .map_err(|e| anyhow::anyhow!("Prover prove request failed: {:#}", e))?;
            anyhow::Ok(Proof {
                journal: p.journal,
                seal: p.seal,
            })
        }

        async fn prove_async(&self, address: Address) -> anyhow::Result<()> {
            let provider = utils::create_provider(self.node_url.clone());
            assert_in_trade_phase(provider, address).await?;
            ProverApiClient::prove_async(&self.client, address)
                .await
                .map_err(|e| anyhow::anyhow!("Prover prove_async request failed: {:#}", e))
        }

        async fn get_proof(&self, address: Address) -> anyhow::Result<Option<Proof>> {
            let address_bytes = address.as_slice();
            let p = self.db.get_proof_by_address(address_bytes).await;
            match p {
                Ok(p) => Ok(Some(Proof {
                    journal: p.proof,
                    seal: p.seal,
                })),
                Err(e) => match e {
                    sqlx::Error::RowNotFound => Ok(None),
                    _ => Err(e.into()),
                },
            }
        }

        async fn get_proof_status(&self, address: Address) -> anyhow::Result<Option<ProofStatus>> {
            let address_bytes = address.as_slice();
            let job = self.db.get_job_by_address(address_bytes).await;
            match job {
                Ok(job) => {
                    let status = match job.status {
                        JobStatus::Created => ProofStatus::Created,
                        JobStatus::InProgress => ProofStatus::InProgress,
                        JobStatus::Completed => ProofStatus::Completed,
                        JobStatus::Errored => ProofStatus::Errored(job.error.unwrap_or_default()),
                    };
                    Ok(Some(status))
                }
                Err(e) => match e {
                    sqlx::Error::RowNotFound => Ok(None),
                    _ => Err(e.into()),
                },
            }
        }
    }
}
