use ethers::contract::abigen;

abigen!(
    TestNFT,
    "./out/TestNFT.sol/TestNFT.json",
    event_derives(serde::Serialize, serde::Deserialize)
);
