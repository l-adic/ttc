use crate::ttc_contract::TopTradingCycle;
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
use tracing::{info, instrument};
use url::Url;

use super::types::{Proof, ProverT};

pub fn create_provider(node_url: Url) -> impl Provider<Http<Client>, Ethereum> + Clone {
    ProviderBuilder::new().on_http(node_url)
}

#[derive(Clone)]
pub struct Prover {
    node_url: Url,
}

impl Prover {
    pub fn new(node_url: &Url) -> Self {
        Self {
            node_url: node_url.clone(),
        }
    }
}

impl ProverT for Prover {
    #[instrument(skip_all, level = "info")]
    async fn prove(&self, address: Address) -> Result<Proof> {
        let provider = create_provider(self.node_url.clone());
        let ttc = TopTradingCycle::new(address, provider);
        let block_number: u64 = {
            let bn = ttc.tradeInitiatedAtBlock().call().await?;
            u64::try_from(bn._0).context("block number is too large")
        }?;
        let mut env = EthEvmEnv::builder()
            .rpc(self.node_url.clone())
            .block_number(block_number)
            .build()
            .await?;

        //  The `with_chain_spec` method is used to specify the chain configuration.
        env = env.with_chain_spec(&ETH_SEPOLIA_CHAIN_SPEC);

        let mut contract = risc0_steel::Contract::preflight(*ttc.address(), &mut env);
        contract
            .call_builder(&TopTradingCycle::getAllTokenPreferencesCall {})
            .call()
            .await?;

        let evm_input = env.into_input().await?;

        info!("Running the guest with the constructed input:");
        let prove_info = tokio::task::spawn_blocking(move || {
            let env = ExecutorEnv::builder()
                .write(&evm_input)?
                .write(&ttc.address())?
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

        let proof = Proof { journal, seal };

        Ok(proof)
    }
}
