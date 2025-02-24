use ethers::contract::abigen;

abigen!(
    TopTradingCycle,
    "./out/TopTradingCycle.sol/TopTradingCycle.json",
    event_derives(serde::Serialize, serde::Deserialize)
);
