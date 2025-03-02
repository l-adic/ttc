use risc0_steel::alloy::sol;

sol!(
    #[sol(rpc, all_derives)]
    TestNFT,
    "../contract/out/TestNFT.sol/TestNFT.json"
);
