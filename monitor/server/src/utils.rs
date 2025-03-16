use risc0_steel::alloy::{
    network::Ethereum,
    providers::{Provider, ProviderBuilder},
    transports::http::{Client, Http},
};
use url::Url;

pub fn create_provider(node_url: Url) -> impl Provider<Http<Client>, Ethereum> + Clone {
    ProviderBuilder::new().on_http(node_url)
}
