use crate::{
    db::Database,
    prover::{remote::Prover, ProverT},
    ttc_contract::TopTradingCycle::{self, PhaseChanged},
};
use alloy::{
    eips::BlockNumberOrTag,
    primitives::Address,
    providers::{ProviderBuilder, WsConnect},
};
use chrono::{TimeZone, Utc};
use futures::StreamExt;
use monitor_common::db::{Job, JobStatus};
use std::collections::HashMap;
use tokio::{sync::Mutex, task::JoinHandle};
use tracing::{debug, span, Level};
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

    // Give the thread 10s to clean itself up before aborting. The reason for this is that
    // we can use this for a graceful shutdown of the monitor thread.
    pub async fn cancel_monitoring(&self, address: Address) -> anyhow::Result<()> {
        let mut events = self.events.lock().await;
        if let Some(handle) = events.remove(&address) {
            let abort_handle = handle.abort_handle();
            let timeout_result =
                tokio::time::timeout(tokio::time::Duration::from_secs(10), handle).await;
            if timeout_result.is_err() {
                abort_handle.abort();
            }
        }
        Ok(())
    }

    pub async fn monitor_trade_phase(
        &self,
        address: Address,
        from_block: u64,
    ) -> anyhow::Result<()> {
        {
            let events = self.events.lock().await;
            if events.contains_key(&address) {
                anyhow::bail!("Already monitoring trade phase for contract {}", address);
            }
        };

        // Clone what we need to move into the spawned task
        let node_url = self.node_url.clone();
        let prover = self.prover.clone();
        let db = self.db.clone();

        let monitor_span = span!(
            Level::DEBUG,
            "event_monitor",
            address = address.to_string().as_str()
        );

        // Spawn the task with cloned values instead of self reference
        let handle = tokio::spawn(async move {
            let result = async {
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
                let mut stream = subscription.into_stream();
                while let Some(result) = stream.next().await {
                    match result {
                        Ok((PhaseChanged { newPhase }, log)) => {
                            debug!(parent: &monitor_span, "TTC contract is in phase {}", newPhase);

                            if newPhase == 2 {
                                let block_number = log.block_number.unwrap() as i64;
                                let block_timestamp = {
                                    let seconds_since_epoch = log.block_timestamp.unwrap() as i64;
                                    Utc.timestamp_opt(seconds_since_epoch, 0).single().unwrap()
                                };

                                debug!(parent: &monitor_span, "TTC contract as moved into trading phase");

                                let job = Job {
                                    address: address.as_slice().to_vec(),
                                    block_number,
                                    block_timestamp,
                                    status: JobStatus::Created,
                                    error: None,
                                    completed_at: None,
                                };
                                db.create_job(&job).await.map_err(anyhow::Error::new)?;
                                debug!(parent: &monitor_span, "Created job for TTC contract. Sending prove request, this could take a while...");
                                prover.prove(*ttc.address()).await?;
                                debug!(parent: &monitor_span, "Successfully processed phase 2, stopping monitor for TTC contract");
                                break; // Stop the stream after processing phase 2
                            }
                        }
                        Err(e) => return Err(anyhow::Error::new(e)),
                    }
                }
                Ok(())
            }
            .await;
            {
                if let Err(e) = &result {
                    tracing::error!(
                        parent: &monitor_span,
                        "Monitor for TTC contract ended with error: {}",
                        e
                    );
                } else {
                    tracing::info!(
                        parent: &monitor_span,
                        "Monitor for TTC contract completed successfully",
                    );
                }
            }

            result
        });
        tracing::info!("Spawned monitor thread for TTC contract {}", address);
        {
            let mut events = self.events.lock().await;
            events.insert(address, handle);
        }
        Ok(())
    }
}
