use clap::Parser;
use time::macros::format_description;
use tracing_subscriber::{
    fmt::{format::FmtSpan, time::UtcTime},
    EnvFilter,
};

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

#[derive(Parser, Debug)]
pub struct Config {
    #[arg(long, name = "node-url", default_value = "http://localhost:8545")]
    pub node_url: String,

    #[arg(long, name = "json-rpc-port", default_value_t = 8546)]
    pub json_rpc_port: usize,
}
