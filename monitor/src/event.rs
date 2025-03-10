use crate::{
    db::{Database, Job, JobStatus},
    env::create_provider,
};
use alloy::{primitives::Address, sol};
use chrono::{TimeZone, Utc};
use futures::StreamExt;
use tracing::debug;
use url::Url;
use TopTradingCycle::PhaseChanged;

sol!(
    #[sol(rpc, all_derives)]
    TopTradingCycle,
    "../contract/out/TopTradingCycle.sol/TopTradingCycle.json"
);

pub async fn monitor_trade_phase(
    node_url: Url,
    db: Database,
    address: Address,
    from_block: u64,
) -> anyhow::Result<()> {
    let provider = create_provider(node_url);
    let ttc = TopTradingCycle::new(address, provider);
    let filter = ttc
        .event_filter::<TopTradingCycle::PhaseChanged>()
        .from_block(from_block);
    let subscription = filter.subscribe().await?;
    subscription
        .into_stream()
        .for_each(|e_event| async {
            match e_event {
                Ok((PhaseChanged { newPhase }, log)) => {
                    if newPhase == 2 {
                        let block_timestamp = {
                            let seconds_since_epoch = log.block_timestamp.unwrap() as i64;
                            Utc.timestamp_opt(seconds_since_epoch, 0).single().unwrap()
                        };
                        debug!("TTC contract {} has moved into trading phase", address);
                        let job = Job {
                            address: address.as_slice().to_vec(),
                            block_number: log.block_number.unwrap() as i64,
                            block_timestamp,
                            status: JobStatus::Created,
                            error: None,
                            completed_at: None,
                        };
                        let res = db.create_job(&job).await;
                        if let Err(err) = res {
                            tracing::error!("Error creating job: {}", err);
                        }
                    }
                }
                Err(err) => {
                    tracing::error!("Error decoding PhaseChanged event: {}", err);
                }
            }
        })
        .await;
    Ok(())
}
