use ethers::{
    middleware::{Middleware, SignerMiddleware},
    providers::{Http, Provider},
    signers::{LocalWallet, Signer},
    types::{Address, U256},
};
use eyre::Result;
use std::sync::Arc;
use std::{collections::HashMap, str::FromStr};
use ttc::strict::{self, Preferences};
use ttc_contract::{
    nft::TestNFT,
    ttc::{TopTradingCycle, top_trading_cycle::TokenReallocation},
};

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

// We want to control how the actors (i.e. contract participants) are created.
// This module forces that only actors with ETH and an NFT are participating,
// which prevents failures for dumb reasons.
mod actor {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    pub struct ActorData {
        pub wallet: LocalWallet,
        pub token_id: U256,
        pub preferences: Vec<U256>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Actor {
        wallet: LocalWallet,
        token_id: U256,
        preferences: Vec<U256>,
    }

    impl Actor {
        pub fn wallet(&self) -> LocalWallet {
            self.wallet.clone()
        }

        pub fn address(&self) -> Address {
            self.wallet.address()
        }

        pub fn token_id(&self) -> U256 {
            self.token_id
        }

        pub fn preferences(&self) -> Vec<U256> {
            self.preferences.clone()
        }

        pub async fn new(
            provider: Arc<Provider<Http>>,
            nft_address: Address,
            owner: LocalWallet,
            data: ActorData,
            nonce: U256,
        ) -> Result<Self> {
            let owner_client = Arc::new(SignerMiddleware::new(provider.clone(), owner.clone()));
            let nft = TestNFT::new(nft_address, owner_client);

            nft.safe_mint(data.wallet.address(), data.token_id)
                .gas(BIG_GAS)
                .nonce(nonce)
                .send()
                .await?
                .await?;

            assert_eq!(
                nft.owner_of(data.token_id).call().await?,
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

// This function calls the solver and produces the data we need to
// submit to the contract
fn reallocate(actors: Vec<Actor>) -> Vec<TokenReallocation> {
    let prefs: strict::Preferences<U256> = {
        let xs = actors
            .iter()
            .map(|a| (a.token_id(), a.preferences()))
            .collect();
        strict::Preferences::new(xs).unwrap()
    };
    let mut g = strict::PreferenceGraph::new(prefs).unwrap();
    let alloc = strict::Allocation::from(g.solve_preferences().unwrap());
    let token_owners: HashMap<U256, Address> = actors
        .into_iter()
        .map(|a| (a.token_id(), a.address()))
        .collect();
    alloc
        .allocation
        .into_iter()
        .map(|(token_id, new_owner)| {
            let new_owner = token_owners.get(&new_owner).unwrap().clone();
            TokenReallocation {
                token_id,
                new_owner,
            }
        })
        .collect()
}

struct TestSetup {
    provider: Arc<Provider<Http>>,
    nft: Address,
    ttc: Address,
    owner: LocalWallet,
    actors: Vec<Actor>,
}

async fn create_actors(
    provider: Arc<Provider<Http>>,
    nft_address: Address,
    owner: LocalWallet,
    actors: Vec<ActorData>,
) -> Result<Vec<Actor>> {
    let start_nonce = provider
        .get_transaction_count(owner.address(), None)
        .await?;

    let futures: Vec<_> = actors
        .into_iter()
        .enumerate()
        .map(|(i, actor_data)| {
            actor::Actor::new(
                provider.clone(),
                nft_address,
                owner.clone(),
                actor_data,
                start_nonce + i,
            )
        })
        .collect();

    let results = futures::future::try_join_all(futures).await?;
    results
        .try_into()
        .map_err(|_| eyre::eyre!("Expected exactly 6 results"))
}

impl TestSetup {
    // Deploy the NFT and TTC contracts and construct the actors.
    async fn new(prefs: strict::Preferences<U256>) -> Result<Self> {
        let provider = {
            let p = Provider::<Http>::try_from(NODE_URL)?;
            Arc::new(p)
        };

        let owner = LocalWallet::from_str(ANVIL_PRIVATE_KEYS[0])?.with_chain_id(ANVIL_CHAIN_ID);
        let client = Arc::new(SignerMiddleware::new(provider.clone(), owner.clone()));
        eprintln!("Deploying NFT");
        let nft = TestNFT::deploy(client.clone(), ())?.send().await?;
        eprintln!("Deploying TTC");
        let ttc = TopTradingCycle::deploy(client.clone(), (nft.address(),))?
            .send()
            .await?;

        let actors: Vec<Actor> = {
            let accounts: Vec<LocalWallet> = ANVIL_PRIVATE_KEYS[1..]
                .into_iter()
                .map(|key| {
                    LocalWallet::from_str(key)
                        .expect("Invalid private key")
                        .with_chain_id(ANVIL_CHAIN_ID)
                })
                .collect();
            let xs: Vec<ActorData> = accounts
                .iter()
                .zip(prefs.prefs)
                .map(|(wallet, (token_id, ps))| ActorData {
                    wallet: wallet.clone(),
                    token_id,
                    preferences: ps.clone(),
                })
                .collect();
            eprintln!("Minting tokens for actors");
            create_actors(provider.clone(), nft.address(), owner.clone(), xs).await?
        };

        Ok(Self {
            provider,
            nft: nft.address(),
            ttc: ttc.address(),
            owner,
            actors,
        })
    }

    // All of the actors deposit their NFTs into the TTC contract
    async fn deposit_tokens(&self) -> Result<()> {
        let futures = self
            .actors
            .iter()
            .map(|actor| {
                let client = Arc::new(SignerMiddleware::new(self.provider.clone(), actor.wallet()));
                let nft = TestNFT::new(self.nft, client.clone());
                let ttc = TopTradingCycle::new(self.ttc, client);
                async move {
                    nft.approve(self.ttc, actor.token_id())
                        .send()
                        .await?
                        .await?;
                    ttc.deposit_nft(actor.token_id()).send().await?.await?;
                    let token_owner = ttc.token_owners(actor.token_id()).call().await?;
                    assert_eq!(
                        token_owner,
                        actor.address(),
                        "Token not deposited correctly in contract!"
                    );
                    Ok::<(), eyre::Report>(())
                }
            })
            .collect::<Vec<_>>();

        futures::future::try_join_all(futures).await?;
        Ok(())
    }

    // All of the actors set their preferences in the TTC contract
    async fn set_preferences(&self) -> Result<()> {
        let futures = self
            .actors
            .clone()
            .into_iter()
            .map(|actor| {
                let client = Arc::new(SignerMiddleware::new(self.provider.clone(), actor.wallet()));
                let ttc = TopTradingCycle::new(self.ttc, client);
                async move {
                    ttc.set_preferences(actor.token_id(), actor.preferences())
                        .gas(BIG_GAS)
                        .send()
                        .await?
                        .await?;
                    let ps = ttc.get_preferences(actor.token_id()).call().await?;
                    assert_eq!(
                        ps,
                        actor.preferences(),
                        "Preferences not set correctly in contract!"
                    );
                    Ok::<(), eyre::Report>(())
                }
            })
            .collect::<Vec<_>>();

        futures::future::try_join_all(futures).await?;
        Ok(())
    }

    // Call the solver and submit the reallocation data to the contract
    async fn reallocate(&self) -> Result<Vec<TokenReallocation>> {
        let client = Arc::new(SignerMiddleware::new(
            self.provider.clone(),
            self.owner.clone(),
        ));
        let ttc = TopTradingCycle::new(self.ttc, client);
        let reallocations: Vec<TokenReallocation> = reallocate(self.actors.clone());
        let stable: Vec<Actor> = {
            self.actors
                .iter()
                .cloned()
                .filter(|a| !reallocations.iter().any(|y| (*y).new_owner == a.address()))
                .collect::<Vec<_>>()
        };

        ttc.reallocate_tokens(reallocations.clone())
            .gas(BIG_GAS)
            .send()
            .await?
            .await?;
        {
            let stable_verification_futures = stable
                .into_iter()
                .map(|a| {
                    let ttc = ttc.clone();
                    async move {
                        let owner = ttc.get_current_owner(a.token_id()).call().await?;
                        assert_eq!(
                            owner,
                            a.address(),
                            "Stable owner didn't maintain their token!"
                        );
                        Ok::<(), eyre::Report>(())
                    }
                })
                .collect::<Vec<_>>();

            futures::future::try_join_all(stable_verification_futures).await?;
        }

        {
            let reallocated_verification_futures = reallocations
                .iter()
                .cloned()
                .map(
                    |TokenReallocation {
                         token_id,
                         new_owner,
                     }| {
                        let ttc = ttc.clone();
                        async move {
                            let owner = ttc.get_current_owner(token_id).call().await?;
                            assert_eq!(owner, new_owner, "Traders didn't get their new token!");
                            Ok::<(), eyre::Report>(())
                        }
                    },
                )
                .collect::<Vec<_>>();

            futures::future::try_join_all(reallocated_verification_futures).await?;
        }
        Ok(reallocations)
    }

    // All of the actors withdraw their tokens, assert that they are getting the right ones!
    async fn withraw(&self, reallocations: Vec<TokenReallocation>) -> Result<()> {
        let token_owners: HashMap<Address, LocalWallet> = self
            .actors
            .iter()
            .map(|a| (a.address(), a.wallet()))
            .collect();
        let futures = reallocations
            .into_iter()
            .map(
                |TokenReallocation {
                     new_owner,
                     token_id,
                 }| {
                    let wallet = token_owners.get(&new_owner).unwrap();
                    let client =
                        Arc::new(SignerMiddleware::new(self.provider.clone(), wallet.clone()));
                    let ttc = TopTradingCycle::new(self.ttc, client);
                    async move {
                        ttc.withdraw_nft(token_id).send().await?.await?;
                        Ok::<(), eyre::Report>(())
                    }
                },
            )
            .collect::<Vec<_>>();

        futures::future::try_join_all(futures).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::arbitrary::Arbitrary;
    use proptest::strategy::{Strategy, ValueTree};
    use proptest::test_runner::TestRunner; // Add this import

    async fn run_test_case(p: Preferences<U256>) -> Result<()> {
        eprintln!("Setting up test environment");
        let setup = TestSetup::new(p).await?;
        eprintln!("Depositing tokens to contract");
        setup.deposit_tokens().await?;
        eprintln!("Declaring preferences in contract");
        setup.set_preferences().await?;
        eprintln!("Computing the reallocation and submitting to contract");
        let reallocs = setup.reallocate().await?;
        eprintln!("Withdrawing tokens from contract back to owners");
        setup.withraw(reallocs).await?;
        Ok(())
    }

    #[cfg(feature = "node_test")]
    #[tokio::test]
    async fn test_ttc_flow() -> Result<()> {
        let test_case = {
            let mut runner = TestRunner::default();
            let strategy = (Preferences::<u64>::arbitrary_with(Some(9..=9)))
                .prop_map(|prefs| prefs.map(U256::from));
            strategy.new_tree(&mut runner).unwrap().current()
        };
        run_test_case(test_case).await?;
        Ok(())
    }
}
