use risc0_steel::alloy::sol;

sol!(
    #[sol(rpc, all_derives)]
    MockVerifier,
    "./out/MockVerifier.sol/MockVerifier.json"
);
