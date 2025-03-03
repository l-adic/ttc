use anyhow::{Ok, Result};
use clap::Parser;
use host::contract::{
    deploy,
    nft::TestNFT,
    ttc::TopTradingCycle::{self},
    Artifacts,
};
use host::prover::{create_provider, Prover, ProverConfig};
use proptest::{
    arbitrary::Arbitrary,
    strategy::{Strategy, ValueTree},
    test_runner::TestRunner,
};
use rand::prelude::SliceRandom;
use risc0_steel::alloy::{primitives::Bytes, signers::Signer, sol_types::SolValue};
use risc0_steel::alloy::{
    primitives::{utils::parse_ether, Address, U256},
    providers::Provider,
    signers::local::PrivateKeySigner,
};
use std::{collections::HashMap, env, str::FromStr, sync::Arc};
use time::macros::format_description;
use tracing::{info, instrument};
use tracing_subscriber::{
    fmt::{format::FmtSpan, time::UtcTime},
    EnvFilter,
};
use ttc::strict::Preferences;
use url::Url;

// We want to control how the actors (i.e. contract participants) are created.
// This module forces that only actors with ETH and an NFT are participating,
// which prevents failures for dumb reasons.
mod actor {
    use super::*;
    use risc0_steel::alloy::{network::TransactionBuilder, rpc::types::TransactionRequest};
    use tracing::info;

    #[derive(Clone)]
    pub struct ActorData {
        pub wallet: PrivateKeySigner,
        pub token: TopTradingCycle::Token,
        pub preferences: Vec<TopTradingCycle::Token>,
    }

    pub fn make_actors_data(
        config: &Config,
        prefs: Preferences<TopTradingCycle::Token>,
    ) -> Vec<ActorData> {
        prefs
            .prefs
            .iter()
            .map(|(token, ps)| {
                let wallet = PrivateKeySigner::random().with_chain_id(Some(config.chain_id));
                ActorData {
                    wallet,
                    token: token.clone(),
                    preferences: ps.clone(),
                }
            })
            .collect()
    }

    #[derive(Clone, PartialEq)]
    pub struct Actor {
        wallet: PrivateKeySigner,
        token: TopTradingCycle::Token,
        preferences: Vec<TopTradingCycle::Token>,
    }

    impl Actor {
        pub fn wallet(&self) -> PrivateKeySigner {
            self.wallet.clone()
        }

        pub fn address(&self) -> Address {
            self.wallet().address()
        }

        pub fn token(&self) -> TopTradingCycle::Token {
            self.token.clone()
        }

        pub fn preferences(&self) -> Vec<TopTradingCycle::Token> {
            self.preferences.clone()
        }

        pub fn with_token(self, token: TopTradingCycle::Token) -> Self {
            Self { token, ..self }
        }

        async fn new(
            config: Config,
            owner: PrivateKeySigner,
            data: ActorData,
            nonce: u64,
        ) -> Result<Self> {
            let provider = create_provider(config.node_url.clone(), owner.clone());

            info!("Fauceting account for {}", data.wallet.address());
            let pending_faucet_tx = {
                let faucet_tx = TransactionRequest::default()
                    .to(data.wallet.address())
                    .value(parse_ether(&config.initial_balance)?)
                    .nonce(nonce + 1)
                    .with_gas_limit(config.max_gas)
                    .with_chain_id(config.chain_id);
                provider.send_transaction(faucet_tx).await?.watch()
            };

            info!(
                "Assigning token ({},{}) to {}",
                data.token.collection,
                data.token.tokenId,
                data.wallet.address()
            );
            let nft = TestNFT::new(data.token.collection, &provider);
            nft.safeMint(data.wallet.address(), data.token.tokenId)
                .gas(config.max_gas)
                .nonce(nonce)
                .send()
                .await?
                .watch()
                .await?;

            pending_faucet_tx.await?;

            assert_eq!(
                nft.ownerOf(data.token.tokenId).call().await?._0,
                data.wallet.address(),
                "The token is assigned to the wrong owner"
            );

            Ok(Self {
                wallet: data.wallet,
                token: data.token,
                preferences: data.preferences,
            })
        }
    }

    pub async fn create_actors(
        config: Config,
        ttc: Address,
        owner: PrivateKeySigner,
        prefs: Preferences<TopTradingCycle::Token>,
    ) -> Result<Vec<Actor>> {
        let provider = create_provider(config.node_url.clone(), owner.clone());
        let start_nonce = provider.get_transaction_count(owner.address()).await?;
        let ds = make_actors_data(&config, prefs);

        let futures: Vec<_> = ds
            .into_iter()
            .enumerate()
            .map(|(i, actor_data)| {
                let ttc = TopTradingCycle::new(ttc, &provider);
                let config = config.clone();
                let owner = owner.clone();
                async move {
                    let a = actor::Actor::new(
                        config,
                        owner,
                        actor_data,
                        start_nonce + 2 * (i as u64), // there are 2 txs, a coin creation and a faucet
                    )
                    .await?;

                    {
                        let contract_hash = ttc.getTokenHash(a.token.clone()).call().await?._0;
                        assert_eq!(
                            contract_hash,
                            a.token.hash(),
                            "We are computing the tokenHash differently than the contract"
                        );
                    }
                    Ok(a)
                }
            })
            .collect();

        let res = futures::future::try_join_all(futures).await?;
        Ok(res)
    }
}

use actor::Actor;

struct TradeResults {
    stable: Vec<Actor>,
    traders: Vec<Actor>,
}

struct TestSetup {
    node_url: Url,
    config: Config,
    ttc: Address,
    owner: PrivateKeySigner,
    actors: Vec<Actor>,
}

fn make_token_preferences(
    nft: Vec<Address>,
    prefs: Preferences<U256>,
) -> Preferences<TopTradingCycle::Token> {
    let mut rng = rand::thread_rng();
    let m = prefs.prefs.keys().fold(HashMap::new(), |mut acc, k| {
        let collection = nft.choose(&mut rng).unwrap();
        acc.insert(k, *collection);
        acc
    });
    prefs.clone().map(|v| {
        let collection = m.get(&v).unwrap();
        TopTradingCycle::Token {
            collection: *collection,
            tokenId: v,
        }
    })
}

impl TestSetup {
    // Deploy the NFT and TTC contracts and construct the actors.
    async fn new(config: &Config, prefs: Preferences<U256>) -> Result<Self> {
        let owner = PrivateKeySigner::from_str(config.owner_key.as_str())?;
        let provider = create_provider(config.node_url.clone(), owner.clone());
        let Artifacts { ttc, nft } = {
            let dev_mode = env::var("RISC0_DEV_MODE").is_ok();
            deploy(provider, dev_mode, config.num_erc721).await
        }?;
        let actors = {
            let prefs = make_token_preferences(nft, prefs);
            actor::create_actors(config.clone(), ttc, owner.clone(), prefs).await
        }?;
        Ok(Self {
            config: config.clone(),
            node_url: config.node_url.clone(),
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
                let nft = TestNFT::new(actor.token().collection, provider.clone());
                let ttc = TopTradingCycle::new(self.ttc, provider);
                async move {
                    nft.approve(self.ttc, actor.token().tokenId)
                        .send()
                        .await?
                        .watch()
                        .await?;
                    ttc.depositNFT(actor.token())
                        .gas(self.config.max_gas)
                        .send()
                        .await?
                        .watch()
                        .await?;
                    Ok(())
                }
            })
            .collect::<Vec<_>>();
        futures::future::try_join_all(approval_futures).await?;

        for actor in self.actors.iter() {
            let provider = create_provider(self.node_url.clone(), actor.wallet());
            let ttc = TopTradingCycle::new(self.ttc, provider);
            {
                let t = ttc
                    .getTokenFromHash(actor.token().hash())
                    .call()
                    .await?
                    .tokenData;
                assert_eq!(
                    t,
                    actor.token(),
                    "Token in contract doesn't match what's expected!"
                );
                let token_owner = ttc.tokenOwners(actor.token().hash()).call().await?._0;
                assert_eq!(token_owner, actor.address(), "Unexpected token owner!")
            }
        }
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
                let prefs = actor
                    .preferences()
                    .iter()
                    .map(|t| t.hash())
                    .collect::<Vec<_>>();
                async move {
                    ttc.setPreferences(actor.token().hash(), prefs.clone())
                        .gas(self.config.max_gas)
                        .send()
                        .await?
                        .watch()
                        .await?;
                    let ps = ttc.getPreferences(actor.token().hash()).call().await?._0;
                    assert_eq!(ps, prefs, "Preferences not set correctly in contract!");
                    info!(
                        "User {} set preferences as {:?}",
                        actor.token(),
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
    async fn reallocate(&self, proof: TopTradingCycle::Journal, seal: Vec<u8>) -> Result<()> {
        let provider = create_provider(self.node_url.clone(), self.owner.clone());
        let ttc = TopTradingCycle::new(self.ttc, provider);
        let journal_data = Bytes::from(proof.abi_encode());
        ttc.reallocateTokens(journal_data, Bytes::from(seal))
            .gas(self.config.max_gas)
            .send()
            .await?
            .watch()
            .await?;
        Ok(())
    }

    async fn results(
        &self,
        original: &[Actor],
        reallocations: &[TopTradingCycle::TokenReallocation],
    ) -> Result<TradeResults> {
        // all the actors who kept their current coins
        let stable: Vec<Actor> = {
            original
                .iter()
                .filter(|a| {
                    reallocations.contains(&TopTradingCycle::TokenReallocation {
                        tokenHash: a.token().hash(),
                        newOwner: a.address(),
                    })
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        let provider = create_provider(self.node_url.clone(), self.owner.clone());
        let ttc = Arc::new(TopTradingCycle::new(self.ttc, provider));
        // all of the actors who made a trade
        let traders = {
            let futures = reallocations
                .iter()
                .cloned()
                .filter_map(|tr| {
                    original
                        .iter()
                        .filter(|a| !stable.contains(a))
                        .find(|a| a.address() == tr.newOwner)
                        .cloned()
                        .map(|a| (tr, a))
                })
                .map(|(tr, a)| {
                    let ttc = Arc::clone(&ttc); // Clone the Arc for each future
                    async move {
                        let token = ttc.getTokenFromHash(tr.tokenHash).call().await?;
                        Ok(a.with_token(token.tokenData))
                    }
                })
                .collect::<Vec<_>>();
            futures::future::try_join_all(futures).await
        }?;
        Ok(TradeResults { stable, traders })
    }

    // All of the actors withdraw their tokens, assert that they are getting the right ones!
    async fn withraw(&self, trade_results: &TradeResults) -> Result<()> {
        info!("assert that the stable actors kept their tokens");
        {
            let futures = trade_results
                .stable
                .iter()
                .map(|actor| {
                    let provider = create_provider(self.node_url.clone(), actor.wallet());
                    let ttc = TopTradingCycle::new(self.ttc, provider);
                    async move {
                        eprintln!(
                            "Withdrawing token {} for {}",
                            actor.token(),
                            actor.address()
                        );
                        ttc.withdrawNFT(actor.token().hash())
                            .send()
                            .await?
                            .watch()
                            .await?;
                        Ok(())
                    }
                })
                .collect::<Vec<_>>();

            futures::future::try_join_all(futures).await?;
        }

        info!("assert that the trading actors get their new tokens");
        {
            let futures = trade_results
                .traders
                .iter()
                .map(|actor| {
                    let provider = create_provider(self.node_url.clone(), actor.wallet());
                    let ttc = TopTradingCycle::new(self.ttc, provider);
                    async move {
                        eprintln!(
                            "Withdrawing token {} for {}",
                            actor.token(),
                            actor.address()
                        );
                        ttc.withdrawNFT(actor.token().hash())
                            .send()
                            .await?
                            .watch()
                            .await?;
                        Ok(())
                    }
                })
                .collect::<Vec<_>>();

            futures::future::try_join_all(futures).await?;
        }

        Ok(())
    }
}

#[instrument(skip_all, level = "info")]
async fn run_test_case(config: Config, p: Preferences<U256>) -> Result<()> {
    //   info!("Setting up test environment for {} actors", p.prefs.len());
    let setup = TestSetup::new(&config, p).await?;
    info!("Depositing tokens to contract");
    setup.deposit_tokens().await?;
    info!("Declaring preferences in contract");
    setup.set_preferences().await?;
    info!("Computing the reallocation");
    let (proof, seal) = {
        let prover_config = ProverConfig {
            node_url: setup.node_url.clone(),
            wallet: setup.owner.clone(),
            ttc: setup.ttc,
        };
        let prover = Prover::new(&prover_config);
        prover.prove().await
    }?;
    setup.reallocate(proof.clone(), seal).await?;
    info!("Withdrawing tokens from contract back to owners");
    let trade_results = setup.results(&setup.actors, &proof.reallocations).await?;
    setup.withraw(&trade_results).await?;
    Ok(())
}

pub fn init_console_subscriber() {
    let timer = UtcTime::new(format_description!(
        "[year]-[month]-[day]T[hour repr:24]:[minute]:[second].[subsecond digits:3]Z"
    ));
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_span_events(FmtSpan::CLOSE)
        .with_timer(timer)
        .with_target(true)
        .with_thread_ids(false)
        .with_line_number(false)
        .with_file(false)
        .with_level(true)
        .with_ansi(true)
        .with_writer(std::io::stdout)
        .init();
}

#[derive(Clone, Parser)]
#[command(author, version, about, long_about = None)]
struct Config {
    /// RPC Node URL
    #[arg(long, default_value = "http://localhost:8545")]
    node_url: Url,

    #[arg(long, default_value_t = 10)]
    max_actors: usize,

    /// Maximum gas limit for transactions
    #[arg(long, default_value_t = 1_000_000u64)]
    max_gas: u64,

    /// Chain ID
    #[arg(long)]
    chain_id: u64,

    /// Initial ETH balance for new accounts
    #[arg(long, default_value = "5")]
    initial_balance: String,

    /// Owner private key (with or without 0x prefix)
    #[arg(long)]
    owner_key: String,

    #[arg(long, name = "num-erc721", default_value_t = 3)]
    num_erc721: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_console_subscriber();
    let cli = Config::parse();

    let test_case = {
        let mut runner = TestRunner::default();
        let strategy = (Preferences::<u64>::arbitrary_with(Some(2..=cli.max_actors)))
            .prop_map(|prefs| prefs.map(U256::from));
        strategy.new_tree(&mut runner).unwrap().current()
    };
    run_test_case(cli, test_case).await?;
    Ok(())
}
