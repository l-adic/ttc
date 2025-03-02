use std::fmt::{self, Debug, Display, Formatter};

use risc0_steel::alloy::{
    primitives::{keccak256, FixedBytes},
    sol,
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
        let mut buf = Vec::new();
        buf.extend_from_slice(self.collection.as_slice());
        buf.extend_from_slice(&self.tokenId.to_be_bytes::<32>());
        keccak256(buf)
    }
}

impl PartialEq for TopTradingCycle::Token {
    fn eq(&self, other: &Self) -> bool {
        self.collection == other.collection && self.tokenId == other.tokenId
    }
}

impl PartialEq for TopTradingCycle::TokenReallocation {
    fn eq(&self, other: &Self) -> bool {
        self.tokenHash == other.tokenHash && self.newOwner == other.newOwner
    }
}
