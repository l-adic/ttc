use std::collections::HashMap;
use tokio::sync::Mutex;

pub struct GasMetrics {
    counter: HashMap<String, u64>,
    histogram: HashMap<String, Vec<u64>>,
}

impl GasMetrics {
    pub fn new() -> Self {
        Self {
            counter: HashMap::new(),
            histogram: HashMap::new(),
        }
    }

    pub fn inc_counter(&mut self, key: &str) {
        let counter = self.counter.entry(key.to_string()).or_default();
        *counter += 1;
    }

    pub fn record_hist(&mut self, key: &str, value: u64) {
        let hist = self.histogram.entry(key.to_string()).or_default();
        hist.push(value);
    }

    pub fn display(&self) {
        println!("Gas Metrics:");
    }
}

impl Default for GasMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for GasMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.counter
            .iter()
            .try_for_each(|(key, value)| writeln!(f, "{}: {}", key, value))?;

        self.histogram.iter().try_for_each(|(key, hist)| {
            let mean = if !hist.is_empty() {
                hist.iter().sum::<u64>() / hist.len() as u64
            } else {
                0
            };

            let median = if !hist.is_empty() {
                let mut hist_clone = hist.clone();
                hist_clone.sort();
                hist_clone[hist_clone.len() / 2]
            } else {
                0
            };

            let max = hist.iter().max().unwrap_or(&0);
            let min = hist.iter().min().unwrap_or(&0);

            writeln!(
                f,
                "{}: mean: {}, median: {}, max: {}, min: {}",
                key, mean, median, max, min
            )
        })?;

        Ok(())
    }
}

pub async fn with_metrics<F, R>(m: &Mutex<GasMetrics>, f: F) -> R
where
    F: FnOnce(&mut GasMetrics) -> R,
{
    let mut metrics = m.lock().await;
    f(&mut metrics)
}
