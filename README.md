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

## Test against local node

See the [node_test](https://github.com/l-adic/ttc/blob/main/.github/workflows/node_test.yml) workflow for how you would set these up and run locally.
