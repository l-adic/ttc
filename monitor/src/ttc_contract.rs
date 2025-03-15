use risc0_steel::alloy::sol;

sol!(
    #[sol(rpc, all_derives)]
    TopTradingCycle,
    "../contract/out/TopTradingCycle.sol/TopTradingCycle.json"
);
