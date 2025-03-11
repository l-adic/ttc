use std::sync::LazyLock;

use alloy::primitives::Address;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct HexAddress(String);

impl From<HexAddress> for Address {
    fn from(hex_address: HexAddress) -> Self {
        let address = hex::decode(hex_address.0).unwrap();
        Address::from_slice(&address)
    }
}

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

pub static JOB_CHANNEL: LazyLock<TypedChannel<HexAddress>> =
    LazyLock::new(|| TypedChannel::new("job_channel"));
