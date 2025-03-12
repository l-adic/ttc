use std::sync::LazyLock;

use alloy::primitives::Address;

impl NotifyPayload for Address {
    fn decode_payload(payload: &str) -> Result<Address, String> {
        let address = hex::decode(payload).map_err(|x| x.to_string())?;
        Ok(Address::from_slice(&address))
    }
}

pub trait NotifyPayload: Sized {
    fn decode_payload(payload: &str) -> Result<Self, String>;
}

#[derive(Clone)]
pub struct TypedChannel<T: NotifyPayload> {
    pub channel_name: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: NotifyPayload> TypedChannel<T> {
    pub fn new(channel_name: &str) -> Self {
        Self {
            channel_name: channel_name.to_string(),
            _phantom: std::marker::PhantomData,
        }
    }
}

pub static JOB_CHANNEL: LazyLock<TypedChannel<Address>> =
    LazyLock::new(|| TypedChannel::new("job_channel"));
