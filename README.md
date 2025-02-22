# Top Trading Cycle

## Background
The [wikipedia article](https://en.wikipedia.org/wiki/Top_trading_cycle) does a good job explaining what the algorithm and setting is, and hints at various generalizations with links in the footnotes.

## Proposed Architecture
1. A Solidity smart contract capabale of 
    - holding NFTs in a custodial mannor (ideally with safe retrieval in case the participant wants to exit before completion)
    - accepting trading preferences
    - "locking down" for a period of time long enough to execute the trading algorithm off chain
    - accepting and validating proofs for the results of the trading algorithm (a "re-allocation")
    - allowing users to withdraw according to re-allocation
2. A rust implementation of the Top Trading Cycle algorithm, and a compatibility layer for inputs/outputs expected by the contract.
3. A zkvm capable of running the rust program and generating an ethereum friendly proof (most likely a groth16 wrapped STARK). E.g. [SP1](https://github.com/succinctlabs/sp1) or [risc0](https://risczero.com/)
4. A simple UI capable of helping a user store/rank/retrieve their NFTs for trading

## NOTES
The part I'm currently most unclear about is how to prove that the input to the trading algorithm is the current smart contract data. AFAIK this is what risc0's [steel](https://github.com/risc0/risc0-ethereum/tree/main/crates/steel) is supposed to help with, it's unclear to me if SP1 has something similar.