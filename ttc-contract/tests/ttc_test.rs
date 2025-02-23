use ethers::{
    middleware::{Middleware, SignerMiddleware},
    providers::{Http, Provider},
    signers::{LocalWallet, Signer},
    types::{Address, U256},
};
use eyre::Result;
use rand::seq::index::sample;
use std::{collections::HashMap, str::FromStr};
use std::{sync::Arc, usize};
use ttc::strict;
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

#[derive(Debug, Clone, PartialEq)]
struct Actor {
    wallet: LocalWallet,
    token_id: U256,
}

impl Actor {
    async fn new(
        provider: Arc<Provider<Http>>,
        nft_address: Address,
        owner: LocalWallet,
        wallet: LocalWallet,
        token_id: U256,
        nonce: U256,
    ) -> Result<Self> {
        let owner_client = Arc::new(SignerMiddleware::new(provider.clone(), owner.clone()));
        let nft = TestNFT::new(nft_address, owner_client);

        nft.safe_mint(wallet.address(), token_id)
            .gas(1_000_000u64)
            .nonce(nonce)
            .send()
            .await?
            .await?;

        assert_eq!(nft.owner_of(token_id).call().await?, wallet.address());

        Ok(Self {
            wallet,
            token_id: token_id,
        })
    }
}

fn example_preferences(actors: [Actor; 6]) -> [Vec<U256>; 6] {
    [
        vec![
            actors[2].token_id,
            actors[1].token_id,
            actors[3].token_id,
            actors[0].token_id,
        ],
        vec![actors[2].token_id, actors[4].token_id, actors[5].token_id],
        vec![actors[2].token_id, actors[0].token_id],
        vec![
            actors[1].token_id,
            actors[4].token_id,
            actors[5].token_id,
            actors[3].token_id,
        ],
        vec![actors[0].token_id, actors[2].token_id],
        vec![
            actors[1].token_id,
            actors[3].token_id,
            actors[4].token_id,
            actors[5].token_id,
        ],
    ]
}

fn example_allocations(actors: [Actor; 6]) -> Vec<TokenReallocation> {
    let prefs: strict::Preferences<U256> = {
        let xs = actors
            .iter()
            .map(|a| a.token_id)
            .zip(example_preferences(actors.clone()))
            .collect();
        strict::Preferences::new(xs).unwrap()
    };
    let mut g = strict::PreferenceGraph::new(prefs).unwrap();
    let alloc = strict::Allocation::from(g.solve_preferences().unwrap());
    let token_owners: HashMap<U256, Address> = actors
        .into_iter()
        .map(|a| (a.token_id, a.wallet.address()))
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
    actors: [Actor; 6],
}

async fn create_actors(
    provider: Arc<Provider<Http>>,
    nft_address: Address,
    owner: LocalWallet,
    actors: [LocalWallet; 6],
) -> Result<[Actor; 6]> {
    let start_nonce = provider
        .get_transaction_count(owner.address(), None)
        .await?;

    let token_ids: Vec<U256> = sample(&mut rand::rng(), usize::MAX, 6)
        .iter()
        .map(|x| U256::from(x))
        .collect();

    let futures: Vec<_> = token_ids
        .iter()
        .zip(actors)
        .enumerate()
        .map(|(i, (id, actor))| {
            Actor::new(
                provider.clone(),
                nft_address,
                owner.clone(),
                actor,
                *id,
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
    async fn new() -> Result<Self> {
        let provider = {
            let p = Provider::<Http>::try_from(NODE_URL)?;
            Arc::new(p)
        };

        let owner = LocalWallet::from_str(ANVIL_PRIVATE_KEYS[0])?.with_chain_id(31337u64);

        let client = Arc::new(SignerMiddleware::new(provider.clone(), owner.clone()));

        let nft = TestNFT::deploy(client.clone(), ())?.send().await?;
        let ttc = TopTradingCycle::deploy(client.clone(), (nft.address(),))?
            .send()
            .await?;

        let actors: [Actor; 6] = {
            let accounts = TryInto::<[&str; 6]>::try_into(&ANVIL_PRIVATE_KEYS[1..7])
                .expect("Not enough private keys")
                .map(|key| {
                    LocalWallet::from_str(key)
                        .expect("Invalid private key")
                        .with_chain_id(31337u64)
                });
            create_actors(provider.clone(), nft.address(), owner.clone(), accounts).await?
        };

        Ok(Self {
            provider,
            nft: nft.address(),
            ttc: ttc.address(),
            owner,
            actors,
        })
    }

    async fn deposit_tokens(&self) -> Result<()> {
        let futures = self
            .actors
            .iter()
            .map(|actor| {
                let client = Arc::new(SignerMiddleware::new(
                    self.provider.clone(),
                    actor.wallet.clone(),
                ));
                let nft = TestNFT::new(self.nft, client.clone());
                let ttc = TopTradingCycle::new(self.ttc, client);
                async move {
                    nft.approve(self.ttc, actor.token_id).send().await?.await?;
                    ttc.deposit_nft(actor.token_id).send().await?.await?;
                    let token_owner = ttc.token_owners(actor.token_id).call().await?;
                    assert_eq!(
                        token_owner,
                        actor.wallet.address(),
                        "Token not deposited correctly in contract!"
                    );
                    Ok::<(), eyre::Report>(())
                }
            })
            .collect::<Vec<_>>();

        futures::future::try_join_all(futures).await?;
        Ok(())
    }

    async fn set_preferences(&self) -> Result<()> {
        let preferences = example_preferences(self.actors.clone());
        let futures = self
            .actors
            .iter()
            .zip(preferences)
            .map(|(actor, prefs)| {
                let client = Arc::new(SignerMiddleware::new(
                    self.provider.clone(),
                    actor.wallet.clone(),
                ));
                let ttc = TopTradingCycle::new(self.ttc, client);
                async move {
                    ttc.set_preferences(actor.token_id, prefs.clone())
                        .gas(1_000_000u64)
                        .send()
                        .await?
                        .await?;
                    let ps = ttc.get_preferences(actor.token_id).call().await?;
                    assert_eq!(ps, prefs, "Preferences not set correctly in contract!");
                    Ok::<(), eyre::Report>(())
                }
            })
            .collect::<Vec<_>>();

        futures::future::try_join_all(futures).await?;
        Ok(())
    }

    async fn reallocate(&self) -> Result<()> {
        let client = Arc::new(SignerMiddleware::new(
            self.provider.clone(),
            self.owner.clone(),
        ));
        let ttc = TopTradingCycle::new(self.ttc, client);
        let reallocations: Vec<TokenReallocation> = example_allocations(self.actors.clone());
        let stable: Vec<Actor> = {
            self.actors
                .iter()
                .cloned()
                .filter(|a| {
                    !reallocations
                        .iter()
                        .any(|y| (*y).new_owner == a.wallet.address())
                })
                .collect::<Vec<_>>()
        };

        ttc.reallocate_tokens(reallocations.clone())
            .gas(1_000_000u64)
            .send()
            .await?
            .await?;
        {
            let stable_verification_futures = stable
                .into_iter()
                .map(|a| {
                    let ttc = ttc.clone();
                    async move {
                        let owner = ttc.get_current_owner(a.token_id).call().await?;
                        assert_eq!(
                            owner,
                            a.wallet.address(),
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
                .into_iter()
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
        Ok(())
    }

    async fn withraw(&self) -> Result<()> {
        let token_owners: HashMap<Address, LocalWallet> = self
            .actors
            .iter()
            .map(|a| (a.wallet.address(), a.wallet.clone()))
            .collect();
        let futures = example_allocations(self.actors.clone())
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

#[tokio::test]
async fn test_deployment() -> Result<()> {
    let setup = TestSetup::new().await?;
    setup.deposit_tokens().await?;
    setup.set_preferences().await?;
    setup.reallocate().await?;
    setup.withraw().await?;
    Ok(())
}
