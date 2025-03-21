use anyhow::{Ok, Result};
use clap::Parser;
use host::contract::{
    nft::TestNFT,
    ttc::TopTradingCycle::{self},
};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use proptest::{
    arbitrary::Arbitrary,
    strategy::{Strategy, ValueTree},
    test_runner::TestRunner,
};
use rand::prelude::SliceRandom;
use risc0_steel::alloy::{
    hex::{self, ToHexExt},
    network::{Ethereum, EthereumWallet},
    primitives::{utils::parse_ether, Address, U256},
    providers::{Provider, ProviderBuilder},
    signers::{k256::FieldBytes, local::PrivateKeySigner},
    transports::http::{Client, Http},
};
use risc0_steel::alloy::{
    primitives::{Bytes, B256},
    signers::Signer,
    sol_types::SolValue,
};
use serde::Serialize;
use std::{collections::HashMap, str::FromStr, thread::sleep, time::Duration};
use time::macros::format_description;
use tracing::{info, instrument};
use tracing_subscriber::{
    fmt::{format::FmtSpan, time::UtcTime},
    EnvFilter,
};
use ttc::strict::Preferences;
use url::Url;

fn create_provider(
    node_url: Url,
    signer: PrivateKeySigner,
) -> impl Provider<Http<Client>, Ethereum> + Clone {
    let wallet = EthereumWallet::from(signer);
    ProviderBuilder::new()
        .with_recommended_fillers() // Add recommended fillers for nonce, gas, etc.
        .wallet(wallet)
        .on_http(node_url)
}

mod deployer {
    use super::*;
    use host::contract::{
        nft::TestNFT, ttc::TopTradingCycle, verifier::MockVerifier, verifier::Verifier,
    };
    use risc0_steel::alloy::{
        network::Ethereum,
        primitives::Address,
        providers::Provider,
        transports::http::{Client, Http},
    };

    pub struct Artifacts {
        pub ttc: Address,
        pub nft: Vec<Address>,
    }

    pub async fn deploy_for_test(
        config: &DeployConfig,
        provider: impl Provider<Http<Client>, Ethereum> + Clone,
        dev_mode: bool,
    ) -> Result<Artifacts> {
        info!("Deploying NFT");

        // Deploy NFTs sequentially to avoid nonce conflicts
        let mut nft = Vec::with_capacity(config.num_erc721);
        for _ in 0..config.num_erc721 {
            let contract = TestNFT::deploy(&provider).await?;
            let address = *contract.address();
            info!("Deployed NFT at {:#}", address);
            nft.push(address);
        }

        info!("Deploying TTC");
        let ttc = {
            let verifier = if dev_mode {
                info!("Deploying MockVerifier");
                *MockVerifier::deploy(&provider).await?.address()
            } else {
                info!("Deploying Groth16Verifier");
                *Verifier::deploy(&provider).await?.address()
            };
            let duration = U256::from(config.phase_duration);
            *TopTradingCycle::deploy(&provider, verifier, duration)
                .await?
                .address()
        };

        Ok(Artifacts { ttc, nft })
    }
}

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
        config: &DemoConfig,
        prefs: Preferences<TopTradingCycle::Token>,
    ) -> Vec<ActorData> {
        prefs
            .prefs
            .iter()
            .map(|(token, ps)| {
                let wallet = PrivateKeySigner::random().with_chain_id(Some(config.base.chain_id));
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

        async fn new(
            config: DemoConfig,
            owner: PrivateKeySigner,
            data: ActorData,
            nonce: u64,
        ) -> Result<Self> {
            let node_url = config.node_url()?;
            let provider = create_provider(node_url, owner.clone());

            info!("Fauceting account for {:#}", data.wallet.address());
            let pending_faucet_tx = {
                let faucet_tx = TransactionRequest::default()
                    .to(data.wallet.address())
                    .value(parse_ether(&config.initial_balance)?)
                    .nonce(nonce + 1)
                    .with_gas_limit(config.base.max_gas)
                    .with_chain_id(config.base.chain_id);
                provider.send_transaction(faucet_tx).await?.watch()
            };

            info!(
                "Assigning token ({:#}, {:#}) with tokenHash {:#} to {:#}",
                data.token.collection,
                data.token.tokenId,
                data.token.hash(),
                data.wallet.address()
            );
            let nft = TestNFT::new(data.token.collection, &provider);
            nft.safeMint(data.wallet.address(), data.token.tokenId)
                .gas(config.base.max_gas)
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
        config: DemoConfig,
        ttc: Address,
        owner: PrivateKeySigner,
        prefs: Preferences<TopTradingCycle::Token>,
    ) -> Result<Vec<Actor>> {
        let node_url = config.node_url()?;
        let provider = create_provider(node_url, owner.clone());
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

        futures::future::try_join_all(futures).await
    }
}

use actor::{Actor, ActorData};
use deployer::{deploy_for_test, Artifacts};

struct TradeResults {
    stable: Vec<Actor>,
    traders: Vec<(Actor, B256)>,
}

struct TestSetup {
    node_url: Url,
    config: DemoConfig,
    ttc: Address,
    owner: PrivateKeySigner,
    actors: Vec<Actor>,
    monitor: HttpClient,
    timeout: Duration,
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
    async fn new(config: &DemoConfig, prefs: Preferences<U256>) -> Result<Self> {
        let owner = PrivateKeySigner::from_str(config.base.owner_key.as_str())?;
        let node_url = config.node_url()?;
        let json = std::fs::read_to_string(config.base.contracts_path())?;
        let addresses: ContractAddresses = serde_json::from_str(&json)?;
        let actors = {
            let prefs = make_token_preferences(addresses.nft, prefs);
            actor::create_actors(config.clone(), addresses.ttc, owner.clone(), prefs).await
        }?;
        let monitor = HttpClientBuilder::default().build(config.monitor_url()?)?;
        Ok(Self {
            config: config.clone(),
            node_url: node_url.clone(),
            ttc: addresses.ttc,
            owner,
            actors,
            monitor,
            timeout: Duration::from_secs(config.prover_timeout),
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
                        .gas(self.config.base.max_gas)
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
                        .gas(self.config.base.max_gas)
                        .send()
                        .await?
                        .watch()
                        .await?;
                    let ps = ttc.getPreferences(actor.token().hash()).call().await?._0;
                    assert_eq!(ps, prefs, "Preferences not set correctly in contract!");
                    info!(
                        "User owning token {:#} set preferences as {:#?}",
                        actor.token().hash(),
                        actor
                            .preferences()
                            .iter()
                            .map(|t| format!("{:#}", t.hash()))
                            .collect::<Vec<_>>()
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
            .gas(self.config.base.max_gas)
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
            let futures = trade_results
                .stable
                .iter()
                .map(|actor| {
                    let provider = create_provider(self.node_url.clone(), actor.wallet());
                    let ttc = TopTradingCycle::new(self.ttc, provider.clone());
                    async move {
                        eprintln!(
                            "Withdrawing token {:#} for existing owner {:#}",
                            actor.token().hash(),
                            actor.address()
                        );
                        ttc.withdrawNFT(actor.token().hash())
                            .gas(self.config.base.max_gas)
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
                .map(|(actor, new_token_hash)| {
                    let provider = create_provider(self.node_url.clone(), actor.wallet());
                    let ttc = TopTradingCycle::new(self.ttc, provider.clone());
                    async move {
                        eprintln!(
                            "Withdrawing token {:#} for new owner {:#}",
                            new_token_hash,
                            actor.address()
                        );
                        ttc.withdrawNFT(*new_token_hash)
                            .gas(self.config.base.max_gas)
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

    async fn poll_until_proof_ready(
        &self,
        address: Address,
    ) -> Result<monitor_api::types::ProofStatus> {
        loop {
            let status =
                monitor_api::rpc::MonitorApiClient::get_proof_status(&self.monitor, address)
                    .await?;
            match status {
                monitor_api::types::ProofStatus::Completed => {
                    return Ok(status);
                }
                monitor_api::types::ProofStatus::Errored(_) => {
                    return Ok(status);
                }
                // not ready yet, delay 5 seconds and try again
                _ => {
                    info!(
                        "Proof for ttc contract {:#} not ready yet, waiting 5 seconds",
                        address
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    // Continue the loop
                }
            }
        }
    }

    async fn advance_phase(&self) -> Result<()> {
        let provider = create_provider(self.node_url.clone(), self.owner.clone());
        let ttc = TopTradingCycle::new(self.ttc, provider);
        ttc.advancePhase().send().await?.watch().await?;
        Ok(())
    }
}

#[instrument(skip_all, level = "info")]
async fn run_test_case(config: DemoConfig, p: Preferences<U256>) -> Result<()> {
    //   info!("Setting up test environment for {} actors", p.prefs.len());
    let setup = TestSetup::new(&config, p).await?;
    let ttc = {
        let node_url = config.node_url()?;
        let provider = create_provider(node_url, setup.owner.clone());
        TopTradingCycle::new(setup.ttc, provider)
    };
    info!("Sending request to watch the contract");
    monitor_api::rpc::MonitorApiClient::watch_contract(&setup.monitor, *ttc.address()).await?;
    info!("Depositing tokens to contract");
    setup.deposit_tokens().await?;
    info!("Advancing phase to Rank");
    setup.advance_phase().await?;
    info!("Declaring preferences in contract");
    setup.set_preferences().await?;
    info!("Advancing phase to Trade");
    setup.advance_phase().await?;
    info!("Computing the reallocation");
    let (proof, seal) = {
        // this is a hack because the monitor probably still hasn't polled the event and started the proof job
        sleep(tokio::time::Duration::from_secs(2));
        info!(
            "Polling the monitor for proof status, timeout is {} seconds",
            setup.timeout.as_secs()
        );
        let status =
            tokio::time::timeout(setup.timeout, setup.poll_until_proof_ready(*ttc.address()))
                .await??;
        if let monitor_api::types::ProofStatus::Errored(e) = status {
            Err(anyhow::anyhow!("Prover errored with message {}", e))
        } else {
            info!("Prover completed successfully");
            let resp =
                monitor_api::rpc::MonitorApiClient::get_proof(&setup.monitor, *ttc.address())
                    .await?;
            let journal = TopTradingCycle::Journal::abi_decode(&resp.journal, true)?;
            Ok((journal, resp.seal))
        }
    }?;
    setup.reallocate(proof.clone(), seal).await?;
    let trade_results = {
        let stable: Vec<Actor> = setup
            .actors
            .iter()
            .filter(|&a| {
                !proof
                    .reallocations
                    .iter()
                    .any(|tr| tr.newOwner == a.address())
            })
            .cloned()
            .collect();
        let traders = setup
            .actors
            .iter()
            .cloned()
            .filter_map(|a| {
                let tr = proof
                    .reallocations
                    .iter()
                    .find(|tr| tr.newOwner == a.address())?;
                Some((a, tr.tokenHash))
            })
            .collect();

        TradeResults { stable, traders }
    };
    info!("Withdrawing tokens from contract back to owners");
    setup.withraw(&trade_results).await?;
    info!("Advancing phase to Cleanup");
    setup.advance_phase().await?;
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
enum Command {
    Deploy(DeployConfig),
    Demo(DemoConfig),
}

#[derive(Clone, Parser, Serialize)]
struct BaseConfig {
    /// Node host
    #[arg(long, env = "NODE_HOST", default_value = "localhost")]
    node_host: String,

    /// Node port
    #[arg(long, env = "NODE_PORT", default_value = "8545")]
    node_port: String,

    /// Owner private key (with or without 0x prefix)
    #[arg(long, env = "OWNER_KEY")]
    owner_key: String,

    /// Chain ID
    #[arg(long, env = "CHAIN_ID")]
    chain_id: u64,

    /// Maximum gas limit for transactions
    #[arg(long, env = "MAX_GAS", default_value_t = 1_000_000u64)]
    max_gas: u64,

    /// Path to contract artifacts
    #[arg(long, env = "ARTIFACTS_DIR", default_value = "deployments")]
    artifacts_dir: String,
}

impl BaseConfig {
    fn node_url(&self) -> Result<Url, url::ParseError> {
        let node_url = format!("http://{}:{}", self.node_host, self.node_port);
        Url::parse(&node_url)
    }

    fn contracts_path(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.artifacts_dir).join("contracts.json")
    }

    fn pre_state_path(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.artifacts_dir).join("pre_state.json")
    }
}

#[derive(Clone, Parser, Serialize)]
struct DeployConfig {
    #[clap(flatten)]
    base: BaseConfig,

    #[arg(long, env = "NUM_ERC721", default_value_t = 3)]
    num_erc721: usize,

    #[arg(long, env = "MOCK_VERIFIER", default_value_t = false)]
    mock_verifier: bool,

    #[arg(long, env = "PHASE_DURATION", default_value_t = 0)]
    phase_duration: u64,
}

#[derive(Clone, Parser, Serialize)]
struct DemoConfig {
    #[clap(flatten)]
    base: BaseConfig,

    #[arg(long, env = "MONITOR_PROTOCOL", default_value = "http")]
    monitor_protocol: String,

    /// Monitor host
    #[arg(long, env = "MONITOR_HOST", default_value = "localhost")]
    monitor_host: String,

    /// Monitor port
    #[arg(long, env = "MONITOR_PORT", default_value = "3030")]
    monitor_port: String,

    #[arg(long, env = "MAX_ACTORS", default_value_t = 10)]
    max_actors: usize,

    /// Initial ETH balance for new accounts
    #[arg(long, env = "INITIAL_BALANCE", default_value = "5")]
    initial_balance: String,

    #[arg(long, env = "PROVER_TIMEOUT", default_value_t = 120)]
    prover_timeout: u64,
}

#[derive(Serialize, serde::Deserialize)]
struct ContractAddresses {
    ttc: Address,
    nft: Vec<Address>,
    verifier: Address,
}

#[derive(Serialize, serde::Deserialize)]
struct TokenOwner {
    token_id: U256,
    token_contract: Address,
    owner: Address,
    owner_key: String,
    preferences: Vec<(Address, U256)>,
}

impl From<Actor> for TokenOwner {
    fn from(actor: Actor) -> Self {
        Self {
            token_id: actor.token().tokenId,
            token_contract: actor.token().collection,
            owner: actor.address(),
            owner_key: actor.wallet().to_field_bytes().encode_hex(),
            preferences: actor
                .preferences()
                .iter()
                .map(|t| (t.collection, t.tokenId))
                .collect(),
        }
    }
}

impl From<TokenOwner> for ActorData {
    fn from(owner: TokenOwner) -> Self {
        let wallet = {
            let slice = hex::decode(owner.owner_key).unwrap();
            let key = FieldBytes::from_slice(&slice);
            PrivateKeySigner::from_field_bytes(key).unwrap()
        };
        let token = TopTradingCycle::Token {
            collection: owner.token_contract,
            tokenId: owner.token_id,
        };
        let preferences = owner
            .preferences
            .iter()
            .map(|(c, t)| TopTradingCycle::Token {
                collection: *c,
                tokenId: *t,
            })
            .collect();
        Self {
            wallet,
            token,
            preferences,
        }
    }
}

impl DeployConfig {
    fn node_url(&self) -> Result<Url, url::ParseError> {
        self.base.node_url()
    }
}

impl DemoConfig {
    fn node_url(&self) -> Result<Url, url::ParseError> {
        self.base.node_url()
    }

    fn monitor_url(&self) -> Result<Url, url::ParseError> {
        let monitor_url = format!(
            "{}://{}:{}",
            self.monitor_protocol, self.monitor_host, self.monitor_port
        );
        Url::parse(&monitor_url)
    }
}

async fn deploy_contracts(config: DeployConfig) -> Result<()> {
    init_console_subscriber();
    info!("{}", serde_json::to_string_pretty(&config).unwrap());

    let owner = PrivateKeySigner::from_str(config.base.owner_key.as_str())?;
    let node_url = config.node_url()?;
    let provider = create_provider(node_url.clone(), owner.clone());

    let Artifacts { ttc, nft } =
        deploy_for_test(&config, provider.clone(), config.mock_verifier).await?;

    // Get verifier address from TTC contract
    let ttc_contract = TopTradingCycle::new(ttc, &provider);
    let verifier = ttc_contract.verifier().call().await?._0;

    let addresses = ContractAddresses { ttc, nft, verifier };
    let json = serde_json::to_string_pretty(&addresses)?;

    // Create deployments directory if it doesn't exist
    if let Some(parent) = std::path::Path::new(&config.base.contracts_path()).parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(config.base.contracts_path(), json)?;
    info!(
        "Contract addresses written to {}",
        config.base.contracts_path().to_str().unwrap()
    );

    Ok(())
}

async fn run_demo(config: DemoConfig) -> Result<()> {
    init_console_subscriber();
    info!("{}", serde_json::to_string_pretty(&config).unwrap());

    let test_case = {
        let mut runner = TestRunner::default();
        let strategy = (Preferences::<u64>::arbitrary_with(Some(2..=config.max_actors)))
            .prop_map(|prefs| prefs.map(U256::from));
        strategy.new_tree(&mut runner).unwrap().current()
    };

    let setup = TestSetup::new(&config, test_case).await?;
    {
        info!(
            "Serializing initial actor state to {}",
            config.base.pre_state_path().to_str().unwrap()
        );
        let actors: Vec<TokenOwner> = setup.actors.iter().map(|a| a.clone().into()).collect();
        let json = serde_json::to_string_pretty(&actors)?;
        std::fs::write(config.base.pre_state_path(), json)?;
    }

    let ttc = {
        let provider = create_provider(setup.node_url.clone(), setup.owner.clone());
        TopTradingCycle::new(setup.ttc, provider)
    };

    info!("Sending request to watch the contract");
    monitor_api::rpc::MonitorApiClient::watch_contract(&setup.monitor, *ttc.address()).await?;
    info!("Depositing tokens to contract");
    setup.deposit_tokens().await?;
    info!("Advancing phase to Rank");
    setup.advance_phase().await?;
    info!("Declaring preferences in contract");
    setup.set_preferences().await?;
    info!("Advancing phase to Trade");
    setup.advance_phase().await?;
    info!("Computing the reallocation");

    let (proof, seal) = {
        sleep(tokio::time::Duration::from_secs(2));
        info!(
            "Polling the monitor for proof status, timeout is {} seconds",
            setup.timeout.as_secs()
        );
        let status =
            tokio::time::timeout(setup.timeout, setup.poll_until_proof_ready(*ttc.address()))
                .await??;
        if let monitor_api::types::ProofStatus::Errored(e) = status {
            Err(anyhow::anyhow!("Prover errored with message {}", e))
        } else {
            info!("Prover completed successfully");
            let resp =
                monitor_api::rpc::MonitorApiClient::get_proof(&setup.monitor, *ttc.address())
                    .await?;
            let journal = TopTradingCycle::Journal::abi_decode(&resp.journal, true)?;
            Ok((journal, resp.seal))
        }
    }?;

    setup.reallocate(proof.clone(), seal).await?;
    let trade_results = {
        let stable: Vec<Actor> = setup
            .actors
            .iter()
            .filter(|&a| {
                !proof
                    .reallocations
                    .iter()
                    .any(|tr| tr.newOwner == a.address())
            })
            .cloned()
            .collect();
        let traders = setup
            .actors
            .iter()
            .cloned()
            .filter_map(|a| {
                let tr = proof
                    .reallocations
                    .iter()
                    .find(|tr| tr.newOwner == a.address())?;
                Some((a, tr.tokenHash))
            })
            .collect();

        TradeResults { stable, traders }
    };

    info!("Withdrawing tokens from contract back to owners");
    setup.withraw(&trade_results).await?;
    info!("Advancing phase to Cleanup");
    setup.advance_phase().await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    match Command::parse() {
        Command::Deploy(config) => deploy_contracts(config).await,
        Command::Demo(config) => run_demo(config).await,
    }
}
