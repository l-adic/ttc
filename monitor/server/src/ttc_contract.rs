use risc0_steel::alloy::sol;

sol!(
    #[sol(rpc, all_derives)]
    ITopTradingCycle,
    "../../contract/out/ITopTradingCycle.sol/ITopTradingCycle.json"
);
