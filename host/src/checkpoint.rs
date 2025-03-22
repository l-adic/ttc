use std::path::{Path, PathBuf};

use crate::actor::{Actor, ActorData};
use crate::{actor::TradeResults, contract::ttc::TopTradingCycle};
use risc0_steel::alloy::primitives::{Address, B256, U256};
use risc0_steel::alloy::{
    hex::{self, ToHexExt},
    signers::{k256::FieldBytes, local::PrivateKeySigner},
};
use serde::Serialize;
use tracing::info;

#[derive(Serialize, serde::Deserialize)]
pub struct ContractAddresses {
    pub ttc: Address,
    pub nft: Vec<Address>,
    pub verifier: Address,
}

#[derive(Serialize, serde::Deserialize)]
struct TradeResultsSerial {
    stable: Vec<TokenOwner>,
    traders: Vec<(TokenOwner, B256)>,
}

impl From<TradeResultsSerial> for TradeResults {
    fn from(serial: TradeResultsSerial) -> Self {
        TradeResults {
            stable: serial.stable.into_iter().map(Actor::from).collect(),
            traders: serial
                .traders
                .into_iter()
                .map(|(t, b)| (Actor::from(t), b))
                .collect(),
        }
    }
}

impl From<TradeResults> for TradeResultsSerial {
    fn from(results: TradeResults) -> Self {
        Self {
            stable: results.stable.into_iter().map(TokenOwner::from).collect(),
            traders: results
                .traders
                .into_iter()
                .map(|(t, b)| (TokenOwner::from(t), b))
                .collect(),
        }
    }
}

#[derive(Serialize, serde::Deserialize)]
pub struct TokenOwner {
    token_id: U256,
    token_contract: Address,
    owner: Address,
    owner_key: String,
    preferences: Vec<(Address, U256)>,
}

impl From<TokenOwner> for Actor {
    fn from(owner: TokenOwner) -> Self {
        let data: ActorData = owner.into();
        Actor {
            wallet: data.wallet,
            token: data.token,
            preferences: data.preferences,
        }
    }
}

impl From<Actor> for TokenOwner {
    fn from(actor: Actor) -> Self {
        Self {
            token_id: actor.token.tokenId,
            token_contract: actor.token.collection,
            owner: actor.address(),
            owner_key: actor.wallet.to_field_bytes().encode_hex(),
            preferences: actor
                .preferences
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

pub enum Checkpoint {
    Deployed(ContractAddresses),
    AssignedTokens(Vec<Actor>),
    Traded(TradeResults),
}

pub struct Checkpointer {
    root_dir: PathBuf,
}

impl Checkpointer {
    pub fn new(root_dir: &Path, ttc_address: Address) -> Self {
        let root_dir = root_dir.join(ttc_address.to_string());
        std::fs::create_dir_all(&root_dir).unwrap();
        Self { root_dir }
    }

    pub fn save(&self, checkpoint: Checkpoint) -> anyhow::Result<()> {
        match checkpoint {
            Checkpoint::Deployed(addresses) => {
                let path = self.root_dir.join("deployed.json");
                info!("Saving deployed contracts to: {:#}", path.display());
                let file = std::fs::File::create(path)?;
                serde_json::to_writer(file, &addresses)?;
            }
            Checkpoint::AssignedTokens(actors) => {
                let path = self.root_dir.join("assigned.json");
                info!("Saving assigned tokens to: {:#}", path.display());
                let file = std::fs::File::create(path)?;
                let serial: Vec<TokenOwner> = actors.into_iter().map(TokenOwner::from).collect();
                serde_json::to_writer(file, &serial)?;
            }
            Checkpoint::Traded(results) => {
                let path = self.root_dir.join("traded.json");
                info!("Saving trade results to: {:#}", path.display());
                let file = std::fs::File::create(path)?;
                let serial: TradeResultsSerial = results.into();
                serde_json::to_writer(file, &serial)?;
            }
        }
        Ok(())
    }

    pub fn load_deployed_contracts(&self) -> anyhow::Result<ContractAddresses> {
        let path = self.root_dir.join("deployed.json");
        info!("Loading deployed contracts from: {:#}", path.display());
        let file = std::fs::File::open(path)?;
        let addresses = serde_json::from_reader(file)?;
        Ok(addresses)
    }

    pub fn load_assigned_tokens(&self) -> anyhow::Result<Vec<Actor>> {
        let path = self.root_dir.join("assigned.json");
        info!("Loading assigned tokens from: {:#}", path.display());
        let file = std::fs::File::open(path)?;
        let serial: Vec<TokenOwner> = serde_json::from_reader(file)?;
        let actors = serial.into_iter().map(Actor::from).collect();
        Ok(actors)
    }

    pub fn load_trade_results(&self) -> anyhow::Result<TradeResults> {
        let path = self.root_dir.join("traded.json");
        info!("Loading trade results from: {:#}", path.display());
        let file = std::fs::File::open(path)?;
        let serial: TradeResultsSerial = serde_json::from_reader(file)?;
        let results: TradeResults = serial.into();
        Ok(results)
    }
}
