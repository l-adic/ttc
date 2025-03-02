use risc0_steel::alloy::sol;

sol!(
    #[sol(rpc, all_derives)]
    Verifier,
    "../contract/out/Verifier.sol/Verifier.json"
);

sol! {
    #[sol(rpc, all_derives)]
    MockVerifier,
    "../contract/out/MockVerifier.sol/MockVerifier.json"
}
