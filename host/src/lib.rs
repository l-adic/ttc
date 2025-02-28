use std::collections::HashMap;

use anyhow::{Context, Ok, Result};
use contract::ttc::{Steel, TopTradingCycle};
use risc0_ethereum_contracts::encode_seal;
use risc0_steel::{
    alloy::{
        eips::BlockNumberOrTag,
        network::{Ethereum, EthereumWallet},
        primitives::{Address, B256, U256},
        providers::{Provider, ProviderBuilder},
        rpc::types::BlockTransactionsKind,
        signers::local::PrivateKeySigner,
        sol_types::SolValue,
        transports::http::{Client, Http},
    },
    ethereum::{EthEvmEnv, ETH_SEPOLIA_CHAIN_SPEC},
    Commitment,
};
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts, VerifierContext};
use tracing::{info, instrument};
use ttc::strict::{self, Preferences};
use ttc_methods::PROVABLE_TTC_ELF;
use url::Url;

pub fn create_provider(
    node_url: Url,
    signer: PrivateKeySigner,
) -> impl Provider<Http<Client>, Ethereum> {
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

pub struct MockProver {
    cfg: ProverConfig,
}

impl MockProver {
    pub fn new(cfg: &ProverConfig) -> Self {
        Self { cfg: cfg.clone() }
    }

    #[instrument(skip_all, level = "info")]
    pub async fn fetch_preferences(&self) -> Result<Vec<TopTradingCycle::TokenPreferences>> {
        let provider = create_provider(self.cfg.node_url.clone(), self.cfg.wallet.clone());
        let ttc = TopTradingCycle::new(self.cfg.ttc, provider);
        let res = ttc.getAllTokenPreferences().call().await?._0;
        Ok(res)
    }

    fn build_owner_dict(prefs: &[TopTradingCycle::TokenPreferences]) -> HashMap<U256, Address> {
        prefs
            .iter()
            .cloned()
            .map(|tp| (tp.tokenId, tp.owner))
            .collect()
    }

    fn reallocate(
        &self,
        depositor_address_from_token_id: HashMap<U256, Address>,
        prefs: Vec<TopTradingCycle::TokenPreferences>,
    ) -> Vec<TopTradingCycle::TokenReallocation> {
        let prefs = {
            let ps = prefs
                .into_iter()
                .map(
                    |TopTradingCycle::TokenPreferences {
                         tokenId,
                         preferences,
                         ..
                     }| { (tokenId, preferences) },
                )
                .collect();
            Preferences::new(ps).unwrap()
        };
        let mut g = strict::PreferenceGraph::new(prefs).unwrap();
        let alloc = strict::Allocation::from(g.solve_preferences().unwrap());
        alloc
            .allocation
            .into_iter()
            .map(|(new_owner, token_id)| {
                let new_owner = depositor_address_from_token_id.get(&new_owner).unwrap();
                let old_owner = depositor_address_from_token_id.get(&token_id).unwrap();
                if new_owner != old_owner {
                    info!(
                        "A trade happened! {} just got token {}",
                        new_owner, token_id
                    );
                }
                TopTradingCycle::TokenReallocation {
                    newOwner: *new_owner,
                    tokenId: token_id,
                }
            })
            .collect()
    }

    async fn make_dummy_commitment(&self) -> Result<Steel::Commitment> {
        let provider = create_provider(self.cfg.node_url.clone(), self.cfg.wallet.clone());
        let b = provider
            .get_block_by_number(BlockNumberOrTag::Latest, BlockTransactionsKind::Hashes)
            .await?
            .unwrap();
        // this is dumb but I'm not sure what the standard way is
        let commitment = {
            let c = Commitment::new(0, b.header.number, b.header.hash, B256::default());
            Steel::Commitment::abi_decode(&c.abi_encode(), true)
        }?;
        Ok(commitment)
    }

    pub async fn prove(&self) -> Result<TopTradingCycle::Journal> {
        let prefs = self.fetch_preferences().await?;
        let depositor_address_from_token_id = Self::build_owner_dict(&prefs);
        let rallocs = self.reallocate(depositor_address_from_token_id, prefs);
        let commitment = self.make_dummy_commitment().await?;
        Ok(TopTradingCycle::Journal {
            commitment,
            reallocations: rallocs,
            ttcContract: self.cfg.ttc,
        })
    }
}
