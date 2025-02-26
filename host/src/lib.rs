use anyhow::{Context, Ok, Result};
use contract::ttc::TopTradingCycle;
use risc0_ethereum_contracts::encode_seal;
use risc0_steel::{
    alloy::{
        network::{Ethereum, EthereumWallet},
        primitives::{Address, U256},
        providers::{Provider, ProviderBuilder},
        signers::local::PrivateKeySigner,
        sol_types::SolValue,
        transports::http::{Client, Http},
    },
    ethereum::{EthEvmEnv, ETH_SEPOLIA_CHAIN_SPEC},
};
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts, VerifierContext};
use std::collections::HashMap;
use ttc::strict::{self, Preferences};
use ttc_methods::PROVABLE_TTC_ELF;
use url::Url;

pub fn create_provider(
    node_url: Url,
    signer: PrivateKeySigner,
) -> impl Provider<Http<Client>, Ethereum> {
    let wallet = EthereumWallet::from(signer);
    ProviderBuilder::new()
       .with_recommended_fillers()  // Add recommended fillers for nonce, gas, etc.
      .wallet(wallet)
      .on_http(node_url)
}

pub struct Prover {
    node_url: Url,
    ttc: Address,
    wallet: PrivateKeySigner,
}

pub struct ProverConfig {
    pub node_url: Url,
    pub ttc: Address,
    pub owner: PrivateKeySigner,
}

impl Prover {
    pub fn new(test_setup: &ProverConfig) -> Self {
        Self {
            node_url: test_setup.node_url.clone(),
            ttc: test_setup.ttc,
            wallet: test_setup.owner.clone(),
        }
    }

    pub async fn fetch_preferences(&self) -> Result<Vec<TopTradingCycle::TokenPreferences>> {
        let provider = create_provider(self.node_url.clone(), self.wallet.clone());
        let ttc = TopTradingCycle::new(self.ttc, provider);
        let res = ttc.getAllTokenPreferences().call().await?._0;
        Ok(res)
    }

    pub async fn prove(&self) -> Result<(TopTradingCycle::Journal, Vec<u8>)> {
        let mut env = EthEvmEnv::builder()
            .rpc(self.node_url.clone())
            .build()
            .await?;

        //  The `with_chain_spec` method is used to specify the chain configuration.
        env = env.with_chain_spec(&ETH_SEPOLIA_CHAIN_SPEC);

        let mut contract = risc0_steel::Contract::preflight(self.ttc.clone(), &mut env);
        let call = TopTradingCycle::getAllTokenPreferencesCall {};
        let preferences = contract.call_builder(&call).call().await?;

        let encoded_preferences = preferences._0.abi_encode();
        let evm_input = env.into_input().await?;


        println!("Running the guest with the constructed input:");
        let ttc = self.ttc.clone();
        let prove_info = tokio::task::spawn_blocking(move || {
            let env = ExecutorEnv::builder()
                .write(&evm_input)?
                .write(&ttc)?
                .write(&encoded_preferences)?
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

        // Decode and log the commitment
        let journal =
            TopTradingCycle::Journal::abi_decode(journal, true).context("invalid journal")?;

        // ABI encode the seal.
        let seal = encode_seal(&receipt).context("invalid receipt")?;

        Ok((journal, seal))
    }

    // pub fn prove_normal(&self, prefs: Vec<TopTradingCycle::TokenPreferences>) -> TopTradingCycle::Journal {
    //     let depositor_address_from_token_id = Self::build_owner_dict(&prefs);
    //     let rallocs = self.reallocate(depositor_address_from_token_id, prefs);
    //     TopTradingCycle::Journal {
    //         reallocations: rallocs,
    //         ttcContract: self.ttc,
    //     }
    // }

    fn build_owner_dict(prefs: &[TopTradingCycle::TokenPreferences]) -> HashMap<U256, Address> {
        prefs
            .iter()
            .cloned()
            .map(|tp| (tp.tokenId, tp.owner))
            .collect()
    }

    // This function calls the solver and produces the data we need to
    // submit to the contract
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
                    eprintln!(
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
}
