use anyhow::Result;
use futures::{future, StreamExt};
use monitor_common::pg_notify::{NotifyPayload, TypedChannel};
use sqlx::{postgres::PgListener, PgPool};
use tokio::sync::mpsc;
use tracing::{error, span, Level};

pub struct PgNotifier<T> {
    notifications: mpsc::UnboundedReceiver<T>,
}

impl<T: NotifyPayload + Send + 'static> PgNotifier<T> {
    pub async fn new(pool: &PgPool, channel: TypedChannel<T>) -> Result<Self> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut listener = PgListener::connect_with(pool).await?;
        listener.listen(&channel.channel_name).await?;

        // Create a span for the entire listener task
        let listener_span = span!(
            Level::INFO,
            "pg_listener",
            channel = %channel.channel_name
        );

        tokio::spawn(async move {
            listener
                .into_stream()
                .filter_map(|message| {
                    let span = span!(
                        parent: &listener_span,
                        Level::DEBUG,
                        "pg_notification",
                        error = tracing::field::Empty
                    );
                    match message {
                        Ok(notification) => {
                            match NotifyPayload::decode_payload(notification.payload()) {
                                Ok(data) => future::ready(Some(data)),
                                Err(e) => {
                                    error!(
                                        parent: &span,
                                        error = %e,
                                        "Deserialization error"
                                    );
                                    future::ready(None)
                                }
                            }
                        }
                        Err(e) => {
                            error!(
                                parent: &span,
                                error = %e,
                                "Error receiving notification"
                            );
                            future::ready(None)
                        }
                    }
                })
                .for_each(|t| {
                    let tx = tx.clone();
                    async move {
                        let span = span!(Level::DEBUG, "send_notification");
                        if let Err(e) = tx.send(t) {
                            error!(
                                parent: &span,
                                error = %e,
                                "Failed to send notification"
                            );
                        }
                    }
                })
                .await
        });
        Ok(Self { notifications: rx })
    }

    pub fn subscribe(self) -> mpsc::UnboundedReceiver<T> {
        self.notifications
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_env::{self, DBConfig, DB};
    use sqlx::PgPool;
    use std::{str::FromStr, time::Duration};

    // Function to publish a notification
    pub async fn publish_notification<T: serde::Serialize>(
        pool: &PgPool,
        channel: &str,
        data: &T,
    ) -> Result<(), anyhow::Error> {
        let payload = serde_json::to_string(data)?;

        // Use SQLx to send a notification
        sqlx::query("SELECT pg_notify($1, $2)")
            .bind(channel)
            .bind(&payload)
            .execute(pool)
            .await?;

        Ok(())
    }

    #[derive(Debug, Clone, PartialEq)]
    struct WrappedInt(i32);

    impl NotifyPayload for WrappedInt {
        fn decode_payload(payload: &str) -> Result<Self, String> {
            let data =
                FromStr::from_str(payload).map_err(|x: std::num::ParseIntError| x.to_string())?;
            Ok(WrappedInt(data))
        }
    }

    #[tokio::test]
    #[ignore = "requires postgres instance"]
    async fn test_sqlx_notify() -> Result<()> {
        app_env::init_console_subscriber();
        eprintln!("[TEST] Starting SQLx notification test");

        let db = DB::new_from_environment().await?;

        let pool = db.pool;

        // Create channel and notifier
        eprintln!("[TEST] Creating typed channel and notifier");
        let channel = TypedChannel::<WrappedInt>::new("test_numbers");
        let notifier = PgNotifier::new(&pool, channel.clone()).await?;
        let mut subscriber = notifier.subscribe();
        eprintln!("[TEST] Notifier created and subscribed");

        // Wait a bit to ensure all setup is complete
        eprintln!("[TEST] Waiting for setup to complete");
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send notifications
        eprintln!("[TEST] Sending test notifications");
        for i in 1..=3 {
            eprintln!("[TEST] Sending notification: {}", i);
            publish_notification(&pool, &channel.channel_name, &i).await?;

            // Add a small delay between notifications
            tokio::time::sleep(Duration::from_millis(50)).await;
            eprintln!("[TEST] Notification {} sent", i);
        }
        eprintln!("[TEST] All notifications sent");

        // Collect received notifications
        eprintln!("[TEST] Collecting received notifications");
        let mut received = Vec::new();
        for i in 0..3 {
            eprintln!("[TEST] Waiting for notification {}", i + 1);
            if let Ok(Some(notification)) =
                tokio::time::timeout(Duration::from_secs(2), subscriber.recv()).await
            {
                eprintln!("[TEST] Received notification: {:?}", notification);
                received.push(notification);
            } else {
                eprintln!("[TEST] Timed out waiting for notification");
                break;
            }
        }

        eprintln!("[TEST] Received {} notifications", received.len());
        assert_eq!(received, vec![WrappedInt(1), WrappedInt(2), WrappedInt(3)]);
        eprintln!("[TEST] Test completed successfully");
        Ok(())
    }
}
