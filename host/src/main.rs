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
use risc0_steel::alloy::{primitives::Bytes, signers::Signer, sol_types::SolValue};
use risc0_steel::alloy::{
    primitives::{utils::parse_ether, Address, U256},
    providers::Provider,
    signers::local::PrivateKeySigner,
};
use std::{env, str::FromStr};
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
            config: Config,
            nft_address: Address,
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
                "Assigning token {} to {}",
                data.token_id,
                data.wallet.address()
            );
            let nft = TestNFT::new(nft_address, &provider);
            nft.safeMint(data.wallet.address(), data.token_id)
                .gas(config.max_gas)
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
            .filter(|a| {
                reallocations.contains(&TopTradingCycle::TokenReallocation {
                    tokenId: a.token_id(),
                    newOwner: a.address(),
                })
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    // all of the actors who made a trade
    let traders: Vec<Actor> = reallocations
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
        .map(|(tr, a)| a.with_token_id(tr.tokenId))
        .collect();
    TradeResults { stable, traders }
}

struct TestSetup {
    node_url: Url,
    config: Config,
    nft: Address,
    ttc: Address,
    owner: PrivateKeySigner,
    actors: Vec<Actor>,
}

async fn create_actors(
    config: Config,
    nft_address: Address,
    owner: PrivateKeySigner,
    actors: Vec<ActorData>,
) -> Result<Vec<Actor>> {
    let provider = create_provider(config.node_url.clone(), owner.clone());

    let start_nonce = provider.get_transaction_count(owner.address()).await?;

    let futures: Vec<_> = actors
        .into_iter()
        .enumerate()
        .map(|(i, actor_data)| {
            actor::Actor::new(
                config.clone(),
                nft_address,
                owner.clone(),
                actor_data,
                start_nonce + 2 * (i as u64), // there are 2 txs, a coin creation and a faucet
            )
        })
        .collect();

    let res = futures::future::try_join_all(futures).await?;
    Ok(res)
}

impl TestSetup {
    // Deploy the NFT and TTC contracts and construct the actors.
    async fn new(config: &Config, actors: Vec<ActorData>) -> Result<Self> {
        let owner = PrivateKeySigner::from_str(config.owner_key.as_str())?;
        let provider = create_provider(config.node_url.clone(), owner.clone());
        let dev_mode = env::var("RISC0_DEV_MODE").is_ok();
        let Artifacts { ttc, nft } = deploy(provider, dev_mode).await?;
        let actors = create_actors(config.clone(), nft, owner.clone(), actors).await?;
        Ok(Self {
            config: config.clone(),
            node_url: config.node_url.clone(),
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
                        .gas(self.config.max_gas)
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
                    info!(
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

    // All of the actors withdraw their tokens, assert that they are getting the right ones!
    async fn withraw(&self, trade_results: &TradeResults) -> Result<()> {
        info!("assert that the stable actors kept their tokens");
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
                        ttc.withdrawNFT(actor.token_id())
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

        info!("assert that the trading actors get their new tokens");
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
                        ttc.withdrawNFT(actor.token_id())
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

fn make_actors_data(config: &Config, prefs: Preferences<U256>) -> Vec<ActorData> {
    let n = prefs.prefs.len();
    let wallets: Vec<PrivateKeySigner> = (0..n)
        .map(|_| PrivateKeySigner::random().with_chain_id(Some(config.chain_id)))
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

#[instrument(skip_all, level = "info")]
async fn run_test_case(config: Config, p: Preferences<U256>) -> Result<()> {
    info!("Setting up test environment for {} actors", p.prefs.len());
    let actors = make_actors_data(&config, p);
    let setup = TestSetup::new(&config, actors).await?;
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
    let trade_results = results(&setup.actors, &proof.reallocations);
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
