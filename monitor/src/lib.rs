mod app_config;
mod db;
mod monitor;
mod prover;
mod ttc_contract;
mod utils;

#[cfg(feature = "server")]
pub mod server {
    pub mod app_config {
        pub use crate::app_config::*;
    }

    pub mod db {
        pub use crate::db::*;
    }

    pub mod monitor {
        pub use crate::monitor::*;
    }

    pub mod prover {
        pub use crate::prover::*;
    }

    pub mod ttc_contract {
        pub use crate::ttc_contract::*;
    }

    pub mod utils {
        pub use crate::utils::*;
    }
}

#[cfg(feature = "client")]
pub mod client {
    pub mod types {
        pub use crate::monitor::types::*;
    }

    pub mod rpc {
        pub use crate::monitor::rpc::*;
    }
}
