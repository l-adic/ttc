use clap::Parser;
use risc0_steel::alloy::primitives::Address;
use serde::Serialize;
use url::Url;

#[derive(Clone, Parser)]
#[command(author, version, about, long_about = None)]
pub enum Command {
    Deploy(DeployConfig),
    Demo(DemoConfig),
}

#[derive(Clone, Parser, Serialize)]
pub struct BaseConfig {
    /// Node host
    #[arg(long, env = "NODE_HOST", default_value = "localhost")]
    pub node_host: String,

    /// Node port
    #[arg(long, env = "NODE_PORT", default_value = "8545")]
    pub node_port: String,

    /// Owner private key (with or without 0x prefix)
    #[arg(long, env = "OWNER_KEY")]
    pub owner_key: String,

    /// Chain ID
    #[arg(long, env = "CHAIN_ID")]
    pub chain_id: u64,

    /// Maximum gas limit for transactions
    #[arg(long, env = "MAX_GAS", default_value_t = 1_000_000u64)]
    pub max_gas: u64,

    /// Path to contract artifacts
    #[arg(long, env = "ARTIFACTS_DIR", default_value = "deployments")]
    pub artifacts_dir: String,
}

impl BaseConfig {
    pub fn node_url(&self) -> Result<Url, url::ParseError> {
        let node_url = format!("http://{}:{}", self.node_host, self.node_port);
        Url::parse(&node_url)
    }
}

#[derive(Clone, Parser, Serialize)]
pub struct DeployConfig {
    #[clap(flatten)]
    pub base: BaseConfig,

    #[arg(long, env = "NUM_ERC721", default_value_t = 3)]
    pub num_erc721: usize,

    #[arg(long, env = "MOCK_VERIFIER", default_value_t = false)]
    pub mock_verifier: bool,

    #[arg(long, env = "PHASE_DURATION", default_value_t = 0)]
    pub phase_duration: u64,
}

#[derive(Clone, Parser, Serialize)]
pub struct DemoConfig {
    #[clap(flatten)]
    pub base: BaseConfig,

    #[arg(long, env = "MONITOR_PROTOCOL", default_value = "http")]
    pub monitor_protocol: String,

    /// Monitor host
    #[arg(long, env = "MONITOR_HOST", default_value = "localhost")]
    pub monitor_host: String,

    /// Monitor port
    #[arg(long, env = "MONITOR_PORT", default_value = "3030")]
    pub monitor_port: String,

    #[arg(long, env = "MAX_ACTORS", default_value_t = 10)]
    pub max_actors: usize,

    /// Initial ETH balance for new accounts
    #[arg(long, env = "INITIAL_BALANCE", default_value = "5")]
    pub initial_balance: String,

    #[arg(long, env = "PROVER_TIMEOUT", default_value_t = 120)]
    pub prover_timeout: u64,

    #[arg(long, env = "TTC_ADDRESS")]
    pub ttc_address: Address,
}

impl DeployConfig {
    pub fn node_url(&self) -> Result<Url, url::ParseError> {
        self.base.node_url()
    }
}

impl DemoConfig {
    pub fn node_url(&self) -> Result<Url, url::ParseError> {
        self.base.node_url()
    }

    pub fn monitor_url(&self) -> Result<Url, url::ParseError> {
        let monitor_url = format!(
            "{}://{}:{}",
            self.monitor_protocol, self.monitor_host, self.monitor_port
        );
        Url::parse(&monitor_url)
    }
}
