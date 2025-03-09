use anyhow::Result;
use serde::de::DeserializeOwned;
use sqlx::{postgres::PgListener, PgPool};
use std::time::Duration;

#[derive(Clone)]
pub struct TypedChannel<T> {
    pub channel_name: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> TypedChannel<T> {
    pub fn new(channel_name: &str) -> Self {
        Self {
            channel_name: channel_name.to_string(),
            _phantom: std::marker::PhantomData,
        }
    }
}

pub struct PgNotifier<T> {
    notifications: tokio::sync::mpsc::UnboundedReceiver<T>,
}

impl<T: DeserializeOwned + Send + 'static> PgNotifier<T> {
    pub async fn new(pool: &PgPool, channel: TypedChannel<T>) -> Result<Self> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut listener = PgListener::connect_with(pool).await?;

        listener.listen(&channel.channel_name).await?;

        tokio::spawn(async move {
            loop {
                match listener.try_recv().await {
                    Ok(Some(notification)) => {
                        match serde_json::from_str::<T>(notification.payload()) {
                            Ok(data) => {
                                if let Err(e) = tx.send(data) {
                                    eprintln!("[ListenerTask] Failed to forward: {:?}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                eprintln!("[ListenerTask] Deserialization error: {}", e);
                            }
                        }
                    }
                    Ok(None) => {
                        eprintln!("[ListenerTask] No notification available");
                    }
                    Err(e) => {
                        eprintln!("[ListenerTask] Error receiving notification: {}", e);

                        // Small delay before retry
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }

            eprintln!("[ListenerTask] Listener task terminated");
        });
        Ok(Self { notifications: rx })
    }

    pub fn subscribe(self) -> tokio::sync::mpsc::UnboundedReceiver<T> {
        self.notifications
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::{DBConfig, DB};
    use sqlx::PgPool;
    use std::time::Duration;

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

    #[tokio::test]
    async fn test_sqlx_notify() -> Result<()> {
        eprintln!("[TEST] Starting SQLx notification test");

        let db = {
            let cfg = DBConfig {
                host: "localhost".to_string(),
                port: 5432,
                user: "postgres".to_string(),
                password: "postgres".to_string(),
                dbname: "postgres".to_string(),
            };
            DB::new(cfg).await?
        };

        let pool = db.pool;

        // Create channel and notifier
        eprintln!("[TEST] Creating typed channel and notifier");
        let channel = TypedChannel::<i32>::new("test_numbers");
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
        assert_eq!(received, vec![1, 2, 3]);
        eprintln!("[TEST] Test completed successfully");
        Ok(())
    }
}
