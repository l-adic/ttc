use anyhow::{Ok, Result};
use clap::Parser;
use host::{
    actor::{self, Actor, TradeResults},
    checkpoint::{self, Checkpoint, Checkpointer, ContractAddresses},
    cli::{Command, DemoConfig, DeployConfig},
    contract::{
        nft::TestNFT,
        ttc::TopTradingCycle::{self},
    },
    deployer::{deploy_for_test, Artifacts},
    env::{create_provider, init_console_subscriber},
};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use proptest::{
    arbitrary::Arbitrary,
    strategy::{Strategy, ValueTree},
    test_runner::TestRunner,
};
use rand::prelude::SliceRandom;
use risc0_steel::alloy::{primitives::Bytes, sol_types::SolValue};
use risc0_steel::alloy::{
    primitives::{utils::parse_ether, Address, U256},
    signers::local::PrivateKeySigner,
};
use std::{collections::HashMap, path::Path, str::FromStr, thread::sleep, time::Duration};
use tracing::info;
use ttc::strict::Preferences;
use url::Url;

struct TestSetup {
    node_url: Url,
    config: DemoConfig,
    ttc: Address,
    owner: PrivateKeySigner,
    actors: Vec<Actor>,
    monitor: HttpClient,
    timeout: Duration,
    checkpointer: Checkpointer,
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
        let checkpointer = {
            let checkpointer_root_dir = Path::new(&config.base.artifacts_dir);
            Checkpointer::new(checkpointer_root_dir, config.ttc_address)
        };
        let addresses = checkpointer.load_deployed_contracts()?;
        let actors = {
            let prefs = make_token_preferences(addresses.nft, prefs);
            let actor_config = actor::Config {
                node_url: node_url.clone(),
                initial_balance: parse_ether(config.initial_balance.as_str()).unwrap(),
                max_gas: config.base.max_gas,
                chain_id: config.base.chain_id,
            };
            actor::create_actors(actor_config, addresses.ttc, owner.clone(), prefs).await
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
            checkpointer,
        })
    }

    async fn new_from_checkpoint(config: &DemoConfig, actors: Vec<Actor>) -> Result<Self> {
        let owner = PrivateKeySigner::from_str(config.base.owner_key.as_str())?;
        let node_url = config.node_url()?;
        let monitor = HttpClientBuilder::default().build(config.monitor_url()?)?;
        let checkpointer = {
            let checkpointer_root_dir = Path::new(&config.base.artifacts_dir);
            Checkpointer::new(checkpointer_root_dir, config.ttc_address)
        };
        let addresses = checkpointer.load_deployed_contracts()?;
        Ok(Self {
            config: config.clone(),
            node_url: node_url.clone(),
            ttc: addresses.ttc,
            owner,
            actors,
            monitor,
            timeout: Duration::from_secs(config.prover_timeout),
            checkpointer,
        })
    }

    async fn deposit_tokens(&self) -> Result<()> {
        // First do all approvals in parallel
        let approval_futures = self
            .actors
            .iter()
            .map(|actor| {
                let provider = create_provider(self.node_url.clone(), actor.wallet.clone());
                let nft = TestNFT::new(actor.token.collection, provider.clone());
                let ttc = TopTradingCycle::new(self.ttc, provider);
                async move {
                    nft.approve(self.ttc, actor.token.tokenId)
                        .send()
                        .await?
                        .watch()
                        .await?;
                    ttc.depositNFT(actor.token.clone())
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
            let provider = create_provider(self.node_url.clone(), actor.wallet.clone());
            let ttc = TopTradingCycle::new(self.ttc, provider);
            {
                let t = ttc
                    .getTokenFromHash(actor.token.hash())
                    .call()
                    .await?
                    .tokenData;
                assert_eq!(
                    t, actor.token,
                    "Token in contract doesn't match what's expected!"
                );
                let token_owner = ttc.tokenOwners(actor.token.hash()).call().await?._0;
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
                let provider = create_provider(self.node_url.clone(), actor.wallet);
                let ttc = TopTradingCycle::new(self.ttc, provider);
                let prefs = actor
                    .preferences
                    .iter()
                    .map(|t| t.hash())
                    .collect::<Vec<_>>();
                async move {
                    ttc.setPreferences(actor.token.hash(), prefs.clone())
                        .gas(self.config.base.max_gas)
                        .send()
                        .await?
                        .watch()
                        .await?;
                    let ps = ttc.getPreferences(actor.token.hash()).call().await?._0;
                    assert_eq!(ps, prefs, "Preferences not set correctly in contract!");
                    info!(
                        "User owning token {:#} set preferences as {:#?}",
                        actor.token.hash(),
                        actor
                            .preferences
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
    async fn reallocate(
        &self,
        proof: TopTradingCycle::Journal,
        seal: Vec<u8>,
    ) -> Result<TradeResults> {
        let provider = create_provider(self.node_url.clone(), self.owner.clone());
        let ttc = TopTradingCycle::new(self.ttc, provider);
        let journal_data = Bytes::from(proof.abi_encode());
        ttc.reallocateTokens(journal_data, Bytes::from(seal))
            .gas(self.config.base.max_gas)
            .send()
            .await?
            .watch()
            .await?;
        let stable: Vec<Actor> = self
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
        let traders = self
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
                    let provider = create_provider(self.node_url.clone(), actor.wallet.clone());
                    let ttc = TopTradingCycle::new(self.ttc, provider.clone());
                    async move {
                        eprintln!(
                            "Withdrawing token {:#} for existing owner {:#}",
                            actor.token.hash(),
                            actor.address()
                        );
                        ttc.withdrawNFT(actor.token.hash())
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
                    let provider = create_provider(self.node_url.clone(), actor.wallet.clone());
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

async fn deploy_contracts(config: DeployConfig) -> Result<ContractAddresses> {
    info!("{}", serde_json::to_string_pretty(&config).unwrap());

    let owner = PrivateKeySigner::from_str(config.base.owner_key.as_str())?;
    let node_url = config.node_url()?;
    let provider = create_provider(node_url.clone(), owner.clone());
    let Artifacts { ttc, nft } = deploy_for_test(
        config.num_erc721,
        config.phase_duration,
        provider.clone(),
        config.mock_verifier,
    )
    .await?;
    let checkpointer = {
        let checkpointer_root_dir = Path::new(&config.base.artifacts_dir);
        Checkpointer::new(checkpointer_root_dir, ttc)
    };
    // Get verifier address from TTC contract
    let ttc_contract = TopTradingCycle::new(ttc, &provider);
    let verifier = ttc_contract.verifier().call().await?._0;
    let addresses = ContractAddresses { ttc, nft, verifier };
    checkpointer.save(checkpoint::Checkpoint::Deployed(addresses.clone()))?;
    Ok(addresses)
}

async fn run_demo(setup: TestSetup) -> Result<()> {
    let ttc = {
        let provider = create_provider(setup.node_url.clone(), setup.owner.clone());
        TopTradingCycle::new(setup.ttc, provider)
    };

    let starting_phase = ttc.currentPhase().call().await?._0;
    info!("TTC contract is currently in phase {}", starting_phase);
    if starting_phase < 2 {
        info!("Sending request to watch the contract");
        monitor_api::rpc::MonitorApiClient::watch_contract(&setup.monitor, *ttc.address()).await?;
    }
    if starting_phase == 0 {
        info!("Depositing tokens to contract");
        setup.deposit_tokens().await?;
        info!("Advancing phase to Rank");
        setup.advance_phase().await?;
    }
    if starting_phase <= 1 {
        info!("Declaring preferences in contract");
        setup.set_preferences().await?;
        info!("Advancing phase to Trade");
        setup.advance_phase().await?;
    }
    let trade_results = if starting_phase <= 2 {
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
                setup.checkpointer.save(Checkpoint::Proved(resp.clone()))?;
                let journal = TopTradingCycle::Journal::abi_decode(&resp.journal, true)?;
                Ok((journal, resp.seal))
            }
        }?;
        let res = setup.reallocate(proof.clone(), seal).await?;
        setup.checkpointer.save(Checkpoint::Traded(res.clone()))?;
        res
    } else {
        setup.checkpointer.load_trade_results()?
    };
    if starting_phase <= 3 {
        info!("Withdrawing tokens from contract back to owners");
        setup.withraw(&trade_results).await?;
        info!("Advancing phase to Cleanup");
        setup.advance_phase().await?;
    }
    if starting_phase == 4 {
        info!("Contract is already closed, no further action needed");
    }
    Ok(())
}

async fn submit_proof(setup: TestSetup) -> Result<()> {
    let ttc = {
        let provider = create_provider(setup.node_url.clone(), setup.owner.clone());
        TopTradingCycle::new(setup.ttc, provider)
    };

    let starting_phase = ttc.currentPhase().call().await?._0;
    if starting_phase != 2 {
        anyhow::bail!("Contract is not in the Trade phase, cannot submit proof");
    }
    let proof = setup.checkpointer.load_proof()?;
    let res = {
        let journal = TopTradingCycle::Journal::abi_decode(&proof.journal, true)?;
        let seal = proof.seal;
        setup.reallocate(journal, seal).await?
    };
    setup.checkpointer.save(Checkpoint::Traded(res.clone()))?;
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    init_console_subscriber();
    match Command::parse() {
        Command::Deploy(config) => {
            let addresses = deploy_contracts(config).await?;
            println!("{}", addresses.ttc);
            Ok(())
        }
        Command::Demo(config) => {
            info!("{}", serde_json::to_string_pretty(&config).unwrap());
            let checkpointer = {
                let checkpointer_root_dir = Path::new(&config.base.artifacts_dir);
                Checkpointer::new(checkpointer_root_dir, config.ttc_address)
            };
            let test_case = {
                let mut runner = TestRunner::default();
                let strategy = (Preferences::<u64>::arbitrary_with(Some(2..=config.max_actors)))
                    .prop_map(|prefs| prefs.map(U256::from));
                strategy.new_tree(&mut runner).unwrap().current()
            };
            let setup = {
                if let std::result::Result::Ok(actors) = checkpointer.load_assigned_tokens() {
                    TestSetup::new_from_checkpoint(&config, actors).await?
                } else {
                    info!(
                        "Setting up test environment for {} actors",
                        test_case.prefs.len()
                    );
                    let setup = TestSetup::new(&config, test_case).await?;
                    checkpointer.save(Checkpoint::AssignedTokens(setup.actors.clone()))?;
                    setup
                }
            };
            run_demo(setup).await
        }
        Command::SubmitProof(config) => {
            info!("{}", serde_json::to_string_pretty(&config).unwrap());
            let checkpointer = {
                let checkpointer_root_dir = Path::new(&config.base.artifacts_dir);
                Checkpointer::new(checkpointer_root_dir, config.ttc_address)
            };
            let setup = if let std::result::Result::Ok(actors) = checkpointer.load_assigned_tokens()
            {
                TestSetup::new_from_checkpoint(&config, actors).await?
            } else {
                anyhow::bail!("No actors found in checkpoint, cannot submit proof");
            };
            submit_proof(setup).await
        }
    }
}
