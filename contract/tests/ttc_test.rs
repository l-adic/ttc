use anyhow::{Ok, Result};
use risc0_steel::alloy::{
    network::EthereumWallet,
    primitives::{Address, U256, utils::parse_ether},
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
};
use std::{collections::HashMap, str::FromStr};
use ttc::strict::{self, Preferences};
use contract::{nft::TestNFT, ttc::TopTradingCycle};
use url::Url;

// I only know these because they are printed when the node starts up, they each come with a balance
// of 10000 ETH.
static ANVIL_PRIVATE_KEYS: [&str; 10] = [
    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
    "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a",
    "0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6",
    "0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a",
    "0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba",
    "0x92db14e403b83dfe3df233f83dfa3a0d7096f21ca9b0d6d6b8d88b2b4ec1564e",
    "0x4bbbf85ce3377467afe5d46f804f221813b2bb87f24d81f60f1fcdbf7cbf4356",
    "0xdbda1821b80551c9d65939329250298aa3472ba22feea921c0cf5d620ea67b97",
    "0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6",
];

static NODE_URL: &str = "http://localhost:8545";

static BIG_GAS: u64 = 1_000_000u64;

static ANVIL_CHAIN_ID: u64 = 31337;

static STARTING_ETH_BALANCE: &str = "5";

fn create_provider(node_url: Url, signer: PrivateKeySigner) -> impl Provider {
    let wallet = EthereumWallet::from(signer);
    ProviderBuilder::new().wallet(wallet).on_http(node_url)
}

// We want to control how the actors (i.e. contract participants) are created.
// This module forces that only actors with ETH and an NFT are participating,
// which prevents failures for dumb reasons.
mod actor {
    use super::*;
    use risc0_steel::alloy::{network::TransactionBuilder, rpc::types::TransactionRequest};

    #[derive(Debug, Clone)]
    pub struct ActorData {
        pub wallet: PrivateKeySigner,
        pub token_id: U256,
        pub preferences: Vec<U256>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Actor {
        wallet: PrivateKeySigner,
        token_id: U256,
        preferences: Vec<U256>,
    }

    impl Actor {
        pub fn wallet(&self) -> PrivateKeySigner {
            self.wallet.clone()
        }

        pub fn address(&self) -> Address {
            self.wallet().address()
        }

        pub fn token_id(&self) -> U256 {
            self.token_id
        }

        pub fn preferences(&self) -> Vec<U256> {
            self.preferences.clone()
        }

        pub fn with_token_id(self, token_id: U256) -> Self {
            Self { token_id, ..self }
        }

        pub async fn new(
            node_url: Url,
            nft_address: Address,
            owner: PrivateKeySigner,
            data: ActorData,
            nonce: u64,
        ) -> Result<Self> {
            let provider = create_provider(node_url.clone(), owner.clone());

            eprintln!("Fauceting account for {}", data.wallet.address());
            let pending_faucet_tx = {
                let faucet_tx = TransactionRequest::default()
                    .to(data.wallet.address())
                    .value(parse_ether(STARTING_ETH_BALANCE)?)
                    .nonce(nonce + 1)
                    .with_gas_limit(BIG_GAS)
                    .with_chain_id(ANVIL_CHAIN_ID);
                provider.send_transaction(faucet_tx).await?.watch()
            };

            eprintln!(
                "Assigning token {} to {}",
                data.token_id,
                data.wallet.address()
            );
            let nft = TestNFT::new(nft_address, &provider);
            nft.safeMint(data.wallet.address(), data.token_id)
                .gas(BIG_GAS)
                .nonce(nonce)
                .send()
                .await?
                .watch()
                .await?;

            pending_faucet_tx.await?;

            assert_eq!(
                nft.ownerOf(data.token_id).call().await?._0,
                data.wallet.address()
            );

            Ok(Self {
                wallet: data.wallet,
                token_id: data.token_id,
                preferences: data.preferences,
            })
        }
    }
}

use actor::{Actor, ActorData};

struct TradeResults {
    stable: Vec<Actor>,
    traders: Vec<Actor>,
}

fn results(
    original: &[Actor],
    reallocations: &[TopTradingCycle::TokenReallocation],
) -> TradeResults {
    // all the actors who kept their current coins
    let stable: Vec<Actor> = {
        original
            .iter()
            .cloned()
            .filter(|a| {
                reallocations.contains(&TopTradingCycle::TokenReallocation {
                    tokenId: a.token_id(),
                    newOwner: a.address(),
                })
            })
            .collect::<Vec<_>>()
    };
    // all of the actors who made a trade
    let traders: Vec<Actor> = reallocations
        .iter()
        .cloned()
        .filter_map(|tr| {
            original
                .iter()
                .cloned()
                .filter(|a| !stable.contains(a))
                .find(|a| a.address() == tr.newOwner)
                .map(|a| (tr, a))
        })
        .map(|(tr, a)| a.with_token_id(tr.tokenId))
        .collect();
    TradeResults { stable, traders }
}

struct TestSetup {
    node_url: Url,
    nft: Address,
    ttc: Address,
    owner: PrivateKeySigner,
    actors: Vec<Actor>,
}

async fn create_actors(
    node_url: Url,
    nft_address: Address,
    owner: PrivateKeySigner,
    actors: Vec<ActorData>,
) -> Result<Vec<Actor>> {
    let provider = create_provider(node_url.clone(), owner.clone());

    let start_nonce = provider.get_transaction_count(owner.address()).await?;

    let futures: Vec<_> = actors
        .into_iter()
        .enumerate()
        .map(|(i, actor_data)| {
            actor::Actor::new(
                node_url.clone(),
                nft_address,
                owner.clone(),
                actor_data,
                start_nonce + 2 * (i as u64), // there are 2 txs, a coin creation and a faucet
            )
        })
        .collect();

    futures::future::try_join_all(futures).await
}

impl TestSetup {
    // Deploy the NFT and TTC contracts and construct the actors.
    async fn new(owner: PrivateKeySigner, actors: Vec<ActorData>) -> Result<Self> {
        let node_url = Url::parse(NODE_URL)?;

        let provider = create_provider(node_url.clone(), owner.clone());

        eprintln!("Deploying NFT");
        let nft = TestNFT::deploy(&provider).await?.address().clone();
        eprintln!("Deploying TTC");
        let ttc = TopTradingCycle::deploy(&provider, nft.clone())
            .await?
            .address()
            .clone();

        let actors = create_actors(node_url.clone(), nft.clone(), owner.clone(), actors).await?;

        Ok(Self {
            node_url,
            nft,
            ttc,
            owner,
            actors,
        })
    }

    async fn deposit_tokens(&self) -> Result<()> {
        // First do all approvals in parallel
        let approval_futures = self
            .actors
            .iter()
            .map(|actor| {
                let provider = create_provider(self.node_url.clone(), actor.wallet());
                let nft = TestNFT::new(self.nft, provider);
                async move {
                    nft.approve(self.ttc, actor.token_id())
                        .send()
                        .await?
                        .watch()
                        .await?;
                    Ok(())
                }
            })
            .collect::<Vec<_>>();
        futures::future::try_join_all(approval_futures).await?;

        // Then do all deposits in parallel
        let deposit_futures = self
            .actors
            .iter()
            .map(|actor| {
                let provider = create_provider(self.node_url.clone(), actor.wallet());
                let ttc = TopTradingCycle::new(self.ttc, provider);
                async move {
                    ttc.depositNFT(actor.token_id())
                        .send()
                        .await?
                        .watch()
                        .await?;
                    let token_owner = ttc.tokenOwners(actor.token_id()).call().await?._0;
                    assert_eq!(
                        token_owner,
                        actor.address(),
                        "Token not deposited correctly in contract!"
                    );
                    Ok(())
                }
            })
            .collect::<Vec<_>>();
        futures::future::try_join_all(deposit_futures).await?;
        Ok(())
    }

    // All of the actors set their preferences in the TTC contract
    async fn set_preferences(&self) -> Result<()> {
        let futures = self
            .actors
            .clone()
            .into_iter()
            .map(|actor| {
                let provider = create_provider(self.node_url.clone(), actor.wallet());
                let ttc = TopTradingCycle::new(self.ttc, provider);
                async move {
                    ttc.setPreferences(actor.token_id(), actor.preferences())
                        .gas(BIG_GAS)
                        .send()
                        .await?
                        .watch()
                        .await?;
                    let ps = ttc.getPreferences(actor.token_id()).call().await?._0;
                    assert_eq!(
                        ps,
                        actor.preferences(),
                        "Preferences not set correctly in contract!"
                    );
                    eprintln!(
                        "User {} set preferences as {:?}",
                        actor.token_id(),
                        actor.preferences()
                    );
                    Ok(())
                }
            })
            .collect::<Vec<_>>();

        futures::future::try_join_all(futures).await?;
        Ok(())
    }

    // Call the solver and submit the reallocation data to the contract
    async fn reallocate(&self, reallocations: &[TopTradingCycle::TokenReallocation]) -> Result<()> {
        let provider = create_provider(self.node_url.clone(), self.owner.clone());
        let ttc = TopTradingCycle::new(self.ttc, provider);
        ttc.reallocateTokens(reallocations.to_vec())
            .gas(BIG_GAS)
            .send()
            .await?
            .watch()
            .await?;
        Ok(())
    }

    // All of the actors withdraw their tokens, assert that they are getting the right ones!
    async fn withraw(&self, trade_results: &TradeResults) -> Result<()> {
        eprintln!("assert that the stable actors kept their tokens");
        {
            let stable_verification_futures = trade_results
                .stable
                .iter()
                .map(|actor| {
                    let provider = create_provider(self.node_url.clone(), actor.wallet());
                    let ttc = TopTradingCycle::new(self.ttc, provider);
                    async move {
                        eprintln!(
                            "Withdrawing token {} for {}",
                            actor.token_id(),
                            actor.address()
                        );
                        ttc.withdrawNFT(actor.token_id().clone())
                            .send()
                            .await?
                            .watch()
                            .await?;
                        Ok(())
                    }
                })
                .collect::<Vec<_>>();

            futures::future::try_join_all(stable_verification_futures).await?;
        }

        eprintln!("assert that the trading actors get their new tokens");
        {
            let stable_verification_futures = trade_results
                .traders
                .iter()
                .map(|actor| {
                    let provider = create_provider(self.node_url.clone(), actor.wallet());
                    let ttc = TopTradingCycle::new(self.ttc, provider);
                    async move {
                        eprintln!(
                            "Withdrawing token {} for {}",
                            actor.token_id(),
                            actor.address()
                        );
                        ttc.withdrawNFT(actor.token_id().clone())
                            .send()
                            .await?
                            .watch()
                            .await?;
                        Ok(())
                    }
                })
                .collect::<Vec<_>>();

            futures::future::try_join_all(stable_verification_futures).await?;
        }

        Ok(())
    }
    // assert that the traders got their new tokens
}

struct Prover {
    node_url: Url,
    ttc: Address,
    wallet: PrivateKeySigner,
}

impl Prover {
    fn new(test_setup: &TestSetup) -> Self {
        Self {
            node_url: test_setup.node_url.clone(),
            ttc: test_setup.ttc,
            wallet: test_setup.owner.clone(),
        }
    }

    async fn fetch_preferences(&self) -> Result<Vec<TopTradingCycle::TokenPreferences>> {
        let provider = create_provider(self.node_url.clone(), self.wallet.clone());
        let ttc = TopTradingCycle::new(self.ttc, provider);
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
                let new_owner = depositor_address_from_token_id
                    .get(&new_owner)
                    .unwrap()
                    .clone();
                let old_owner = depositor_address_from_token_id
                    .get(&token_id)
                    .unwrap()
                    .clone();
                if new_owner != old_owner {
                    eprintln!(
                        "A trade happened! {} just got token {}",
                        new_owner, token_id
                    );
                }
                TopTradingCycle::TokenReallocation {
                    newOwner: new_owner,
                    tokenId: token_id,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::{
        arbitrary::Arbitrary,
        strategy::{Strategy, ValueTree},
        test_runner::TestRunner,
    };
    use risc0_steel::alloy::signers::Signer; // Add this import

    fn make_actors_data(prefs: Preferences<U256>) -> Vec<ActorData> {
        let n = prefs.prefs.len();
        let wallets: Vec<PrivateKeySigner> = (0..n)
            .map(|_| PrivateKeySigner::random().with_chain_id(Some(ANVIL_CHAIN_ID)))
            .collect();
        wallets
            .iter()
            .zip(prefs.prefs)
            .map(|(wallet, (token_id, ps))| ActorData {
                wallet: wallet.clone(),
                token_id,
                preferences: ps.clone(),
            })
            .collect()
    }

    async fn run_test_case(p: Preferences<U256>) -> Result<()> {
        eprintln!("Setting up test environment for {} actors", p.prefs.len());
        let actors = make_actors_data(p);
        let owner = PrivateKeySigner::from_str(ANVIL_PRIVATE_KEYS[0])?;
        let setup = TestSetup::new(owner, actors).await?;
        eprintln!("Depositing tokens to contract");
        setup.deposit_tokens().await?;
        eprintln!("Declaring preferences in contract");
        setup.set_preferences().await?;
        eprintln!("Computing the reallocation");
        let reallocs = {
            let prover = Prover::new(&setup);
            let prefs = prover.fetch_preferences().await?;
            let owner_dict = Prover::build_owner_dict(&prefs);
            prover.reallocate(owner_dict, prefs)
        };
        setup.reallocate(&reallocs).await?;
        eprintln!("Withdrawing tokens from contract back to owners");
        let trade_results = results(&setup.actors, &reallocs);
        setup.withraw(&trade_results).await?;
        Ok(())
    }

    #[cfg(feature = "node_test")]
    #[tokio::test]
    async fn test_ttc_flow() -> Result<()> {
        let test_case = {
            let mut runner = TestRunner::default();
            let strategy = (Preferences::<u64>::arbitrary_with(Some(2..=20)))
                .prop_map(|prefs| prefs.map(U256::from));
            strategy.new_tree(&mut runner).unwrap().current()
        };
        run_test_case(test_case).await?;
        Ok(())
    }
}
