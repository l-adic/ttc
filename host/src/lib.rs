use anyhow::{Ok, Result};
use contract::ttc::TopTradingCycle;
use risc0_steel::alloy::{
    network::EthereumWallet,
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
};
use std::collections::HashMap;
use ttc::strict::{self, Preferences};
use url::Url;

pub fn create_provider(node_url: Url, signer: PrivateKeySigner) -> impl Provider {
    let wallet = EthereumWallet::from(signer);
    ProviderBuilder::new().wallet(wallet).on_http(node_url)
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

    pub fn prove(&self, prefs: Vec<TopTradingCycle::TokenPreferences>) -> TopTradingCycle::Journal {
        let depositor_address_from_token_id = Self::build_owner_dict(&prefs);
        let rallocs = self.reallocate(depositor_address_from_token_id, prefs);
        TopTradingCycle::Journal {
            reallocations: rallocs,
            ttcContract: self.ttc,
        }
    }

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
