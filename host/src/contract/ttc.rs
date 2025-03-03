use std::{
    fmt::{self, Debug, Display, Formatter},
    hash::{Hash, Hasher},
};

use risc0_steel::alloy::{
    primitives::{keccak256, FixedBytes},
    sol,
    sol_types::SolValue,
};

sol!(
    #[sol(rpc, all_derives)]
    TopTradingCycle,
    "../contract/out/TopTradingCycle.sol/TopTradingCycle.json"
);

impl Debug for TopTradingCycle::Token {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Token {{ collection: {:?}, tokenId: {:?} }}",
            self.collection, self.tokenId
        )
    }
}

impl Display for TopTradingCycle::Token {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Token {{ collection: {:?}, tokenId: {:?} }}",
            self.collection, self.tokenId
        )
    }
}

impl TopTradingCycle::Token {
    // This should use the equivalent of Solidity abi.encodePacked
    pub fn hash(&self) -> FixedBytes<32> {
        keccak256(self.abi_encode_packed())
    }
}

impl PartialEq for TopTradingCycle::Token {
    fn eq(&self, other: &Self) -> bool {
        self.collection == other.collection && self.tokenId == other.tokenId
    }
}

impl Eq for TopTradingCycle::Token {}

impl Hash for TopTradingCycle::Token {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.collection.hash(state);
        self.tokenId.hash(state);
    }
}

impl PartialEq for TopTradingCycle::TokenReallocation {
    fn eq(&self, other: &Self) -> bool {
        self.tokenHash == other.tokenHash && self.newOwner == other.newOwner
    }
}
