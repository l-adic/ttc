use super::{
    rpc::ProverApiClient,
    types::{AsyncProverT, Proof, ProverT},
};
use crate::{ttc_contract, utils};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use risc0_steel::alloy::{
    network::Ethereum,
    primitives::Address,
    providers::Provider,
    transports::http::{Client, Http},
};
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
}

impl Prover {
    pub fn new(node_url: Url, prover_url: Url, prover_timeout: u64) -> anyhow::Result<Self> {
        let client = HttpClientBuilder::default()
            .request_timeout(std::time::Duration::from_secs(prover_timeout))
            .build(prover_url)?;
        Ok(Self { node_url, client })
    }

    pub async fn get_image_id_contract(&self) -> anyhow::Result<String> {
        ProverApiClient::get_image_id_contract(&self.client)
            .await
            .map_err(|e| anyhow::anyhow!("Prover get_image_id_contract request failed: {:#}", e))
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
}

impl AsyncProverT for Prover {
    async fn prove_async(&self, address: Address) -> anyhow::Result<()> {
        let provider = utils::create_provider(self.node_url.clone());
        assert_in_trade_phase(provider, address).await?;
        ProverApiClient::prove_async(&self.client, address)
            .await
            .map_err(|e| anyhow::anyhow!("Prover prove_async request failed: {:#}", e))
    }
}
