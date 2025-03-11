use crate::{
    db::{Database, Job, JobStatus},
    prover::{remote::Prover, ProverT},
    ttc_contract::TopTradingCycle::{self, PhaseChanged},
};
use alloy::{
    eips::BlockNumberOrTag,
    primitives::Address,
    providers::{ProviderBuilder, WsConnect},
};
use chrono::{TimeZone, Utc};
use futures::TryStreamExt;
use futures::{future, StreamExt};
use std::collections::HashMap;
use tokio::{sync::Mutex, task::JoinHandle};
use tracing::debug;
use url::Url;

#[allow(async_fn_in_trait)]
pub trait EventsManagerT {
    async fn monitor_trade_phase(&self, address: Address, from_block: u64) -> anyhow::Result<()>;
    async fn cancel_monitoring(&self, address: Address) -> anyhow::Result<()>;
}

pub struct EventsManager {
    events: Mutex<HashMap<Address, JoinHandle<anyhow::Result<()>>>>,
    node_url: Url,
    prover: Prover,
    db: Database,
}

impl EventsManager {
    pub fn new(node_url: Url, prover: Prover, db: Database) -> Self {
        Self {
            events: Mutex::new(HashMap::new()),
            node_url,
            prover,
            db,
        }
    }

    pub async fn cancel_monitoring(&self, address: Address) -> anyhow::Result<()> {
        let mut events = self.events.lock().await;
        if let Some(handle) = events.remove(&address) {
            handle.abort();
        }
        Ok(())
    }

    pub async fn monitor_trade_phase(
        &self,
        address: Address,
        from_block: u64,
    ) -> anyhow::Result<()> {
        let mut events = self.events.lock().await;
        if events.contains_key(&address) {
            anyhow::bail!("Already monitoring trade phase for contract {}", address);
        }

        // Clone what we need to move into the spawned task
        let node_url = self.node_url.clone();
        let prover = self.prover.clone();
        let db = self.db.clone();

        // Spawn the task with cloned values instead of self reference
        let handle = tokio::spawn(async move {
            let provider = {
                let rpc_url = format!(
                    "ws://{}:{}",
                    node_url.host_str().unwrap(),
                    node_url.port().unwrap()
                );
                let ws = WsConnect::new(rpc_url);
                ProviderBuilder::new().on_ws(ws).await?
            };
            let ttc = TopTradingCycle::new(address, provider);
            let filter = ttc
                .event_filter::<TopTradingCycle::PhaseChanged>()
                .from_block(from_block)
                .to_block(BlockNumberOrTag::Latest);
            let subscription = filter.subscribe().await.map_err(anyhow::Error::new)?;
            let result = subscription
                .into_stream()
                .map(|x| x.map_err(anyhow::Error::new))
                .try_take_while(|(PhaseChanged { newPhase }, _)| future::ready(Ok(*newPhase <= 2)))
                .try_for_each(|(PhaseChanged { newPhase }, log)| {
                    let block_number = log.block_number.unwrap() as i64;
                    let db = db.clone();
                    let ttc = ttc.clone();
                    let prover = prover.clone();
                    async move {
                        if newPhase == 2 {
                            let block_timestamp = {
                                let seconds_since_epoch = log.block_timestamp.unwrap() as i64;
                                Utc.timestamp_opt(seconds_since_epoch, 0).single().unwrap()
                            };
                            debug!("TTC contract {} has moved into trading phase", address);
                            let job = Job {
                                address: address.as_slice().to_vec(),
                                block_number,
                                block_timestamp,
                                status: JobStatus::Created,
                                error: None,
                                completed_at: None,
                            };
                            db.create_job(&job).await.map_err(anyhow::Error::new)?;
                            prover.prove_async(*ttc.address()).await
                        } else {
                            debug!("TTC contract {} has moved into phase {}", address, newPhase);
                            anyhow::Ok(())
                        }
                    }
                })
                .await;
            match result {
                Ok(()) => {
                    debug!(
                        "Monitoring trade phase for TTC contract {} completed",
                        address
                    );
                    Ok(())
                }
                Err(err) => {
                    debug!(
                        "Error monitoring trade phase for TTC contract {}: {:#}",
                        address, err
                    );
                    Err(err)
                }
            }
        });
        debug!("Spawned monitor thread for TTC contract {}", address);
        events.insert(address, handle);
        Ok(())
    }
}
