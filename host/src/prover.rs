use crate::contract::ttc::TopTradingCycle;
use anyhow::{Context, Ok, Result};
use methods::PROVABLE_TTC_ELF;
use risc0_ethereum_contracts::encode_seal;
use risc0_steel::{
    alloy::{
        network::{Ethereum, EthereumWallet},
        primitives::Address,
        providers::{Provider, ProviderBuilder},
        signers::local::PrivateKeySigner,
        sol_types::SolValue,
        transports::http::{Client, Http},
    },
    ethereum::{EthEvmEnv, ETH_SEPOLIA_CHAIN_SPEC},
};
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts, VerifierContext};
use tracing::{info, instrument};
use url::Url;

pub fn create_provider(
    node_url: Url,
    signer: PrivateKeySigner,
) -> impl Provider<Http<Client>, Ethereum> + Clone {
    let wallet = EthereumWallet::from(signer);
    ProviderBuilder::new()
        .with_recommended_fillers() // Add recommended fillers for nonce, gas, etc.
        .wallet(wallet)
        .on_http(node_url)
}

#[derive(Clone)]
pub struct ProverConfig {
    pub node_url: Url,
    pub ttc: Address,
    pub wallet: PrivateKeySigner,
}

pub struct Prover {
    cfg: ProverConfig,
}

impl Prover {
    pub fn new(cfg: &ProverConfig) -> Self {
        Self { cfg: cfg.clone() }
    }

    #[instrument(skip_all, level = "info")]
    pub async fn prove(&self) -> Result<(TopTradingCycle::Journal, Vec<u8>)> {
        let mut env = EthEvmEnv::builder()
            .rpc(self.cfg.node_url.clone())
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
        let journal = &receipt.journal.bytes;

        // HOLD ONTO YOUR BUTTS, this Journal type better match the one in guest!
        let journal = TopTradingCycle::Journal::abi_decode(journal, true)
            .context("Shared journal doesn't match contract journal")?;

        // ABI encode the seal.
        let seal = encode_seal(&receipt).context("invalid receipt")?;

        Ok((journal, seal))
    }
}
