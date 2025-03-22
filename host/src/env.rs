use risc0_steel::alloy::{
    network::{Ethereum, EthereumWallet},
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    transports::http::{Client, Http},
};
use time::macros::format_description;
use tracing_subscriber::{
    fmt::{format::FmtSpan, time::UtcTime},
    EnvFilter,
};
use url::Url;

/// Initialize the console subscriber for logging
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

pub fn create_provider(
    node_url: Url,
    signer: PrivateKeySigner,
) -> impl Provider<Http<Client>, Ethereum> + Clone {
    let wallet = EthereumWallet::from(signer);
    ProviderBuilder::new()
        .with_recommended_fillers() // Add recommended fillers for nonce, gas, etc.
        .wallet(wallet)
        .on_http(node_url)
}
