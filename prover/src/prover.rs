pub mod ttc_contract {
    use risc0_steel::alloy::sol;

    sol!(
        #[sol(rpc, all_derives)]
        TopTradingCycle,
        "../contract/out/TopTradingCycle.sol/TopTradingCycle.json"
    );
}

use anyhow::{Context, Ok, Result};
use methods::PROVABLE_TTC_ELF;
use risc0_ethereum_contracts::encode_seal;
use risc0_steel::{
    alloy::{
        network::Ethereum,
        primitives::Address,
        providers::{Provider, ProviderBuilder},
        transports::http::{Client, Http},
    },
    ethereum::{EthEvmEnv, ETH_SEPOLIA_CHAIN_SPEC},
};
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts, VerifierContext};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};
use ttc_contract::TopTradingCycle;
use url::Url;

pub fn create_provider(node_url: Url) -> impl Provider<Http<Client>, Ethereum> + Clone {
    ProviderBuilder::new().on_http(node_url)
}

#[derive(Clone)]
pub struct ProverConfig {
    pub node_url: Url,
    pub ttc: Address,
}

pub struct Prover {
    cfg: ProverConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    pub journal: Vec<u8>,
    pub seal: Vec<u8>,
}

impl Prover {
    pub fn new(cfg: &ProverConfig) -> Self {
        Self { cfg: cfg.clone() }
    }

    #[instrument(skip_all, level = "info")]
    pub async fn prove(&self) -> Result<Proof> {
        let block_number: u64 = {
            let provider = create_provider(self.cfg.node_url.clone());
            let ttc = TopTradingCycle::new(self.cfg.ttc, provider);
            let bn = ttc.tradeInitiatedAtBlock().call().await?;
            u64::try_from(bn._0).context("block number is too large")
        }?;
        let mut env = EthEvmEnv::builder()
            .rpc(self.cfg.node_url.clone())
            .block_number(block_number)
            .build()
            .await?;

        //  The `with_chain_spec` method is used to specify the chain configuration.
        env = env.with_chain_spec(&ETH_SEPOLIA_CHAIN_SPEC);

        let mut contract = risc0_steel::Contract::preflight(self.cfg.ttc, &mut env);
        contract
            .call_builder(&TopTradingCycle::getAllTokenPreferencesCall {})
            .call()
            .await?;

        let evm_input = env.into_input().await?;

        info!("Running the guest with the constructed input:");
        let ttc = self.cfg.ttc;
        let prove_info = tokio::task::spawn_blocking(move || {
            let env = ExecutorEnv::builder()
                .write(&evm_input)?
                .write(&ttc)?
                .build()
                .unwrap();

            default_prover().prove_with_ctx(
                env,
                &VerifierContext::default(),
                PROVABLE_TTC_ELF,
                &ProverOpts::groth16(),
            )
        })
        .await?
        .context("failed to create proof")?;

        let receipt = prove_info.receipt;
        let seal = encode_seal(&receipt).context("invalid receipt")?;
        let journal = receipt.journal.bytes;

        Ok(Proof { journal, seal })
    }
}
