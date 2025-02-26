use risc0_steel::alloy::sol;

sol!(
    #[sol(rpc, all_derives)]
    TopTradingCycle,
    "./out/TopTradingCycle.sol/TopTradingCycle.json"
);

impl PartialEq for TopTradingCycle::TokenReallocation {
    fn eq(&self, other: &Self) -> bool {
        self.tokenId == other.tokenId && self.newOwner == other.newOwner
    }
}
