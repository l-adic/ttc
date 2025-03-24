use crate::contract::{nft::TestNFT, ttc::ITopTradingCycle};
use anyhow::{Ok, Result};
use risc0_steel::alloy::{network::TransactionBuilder, rpc::types::TransactionRequest};
use risc0_steel::alloy::{
    primitives::{Address, B256, U256},
    providers::Provider,
    signers::{local::PrivateKeySigner, Signer},
};
use tracing::info;
use ttc::strict::Preferences;
use url::Url;

#[derive(Clone)]
pub struct Config {
    pub node_url: Url,
    pub initial_balance: U256,
    pub max_gas: u64,
    pub chain_id: u64,
}

#[derive(Clone)]
pub struct ActorData {
    pub wallet: PrivateKeySigner,
    pub token: ITopTradingCycle::Token,
    pub preferences: Vec<ITopTradingCycle::Token>,
}

pub fn make_actors_data(
    config: &Config,
    prefs: Preferences<ITopTradingCycle::Token>,
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
    pub wallet: PrivateKeySigner,
    pub token: ITopTradingCycle::Token,
    pub preferences: Vec<ITopTradingCycle::Token>,
}

impl Actor {
    pub fn address(&self) -> Address {
        self.wallet.address()
    }

    async fn new(
        config: Config,
        owner: PrivateKeySigner,
        data: ActorData,
        nonce: u64,
    ) -> Result<Self> {
        let node_url = config.node_url;
        let provider = crate::env::create_provider(node_url, owner.clone());

        info!("Fauceting account for {:#}", data.wallet.address());
        let pending_faucet_tx = {
            let faucet_tx = TransactionRequest::default()
                .to(data.wallet.address())
                .value(config.initial_balance)
                .nonce(nonce + 1)
                .with_gas_limit(config.max_gas)
                .with_chain_id(config.chain_id);
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
    prefs: Preferences<ITopTradingCycle::Token>,
) -> Result<Vec<Actor>> {
    let provider = crate::env::create_provider(config.node_url.clone(), owner.clone());
    let start_nonce = provider.get_transaction_count(owner.address()).await?;
    let ds = make_actors_data(&config, prefs);

    let futures: Vec<_> = ds
        .into_iter()
        .enumerate()
        .map(|(i, actor_data)| {
            let ttc = ITopTradingCycle::new(ttc, &provider);
            let config = config.clone();
            let owner = owner.clone();
            async move {
                let a = Actor::new(
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

#[derive(Clone)]
pub struct TradeResults {
    pub stable: Vec<Actor>,
    pub traders: Vec<(Actor, B256)>,
}
