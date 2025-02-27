use risc0_steel::alloy::sol;

sol!(
    #[sol(rpc, all_derives)]
    Verifier,
    "./out/Verifier.sol/Verifier.json"
);
