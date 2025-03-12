# Top Trading Cycle

## Overview
A group of people owning various ERC-721 (i.e non-fungible) tokens would like to trade amongst each other. They would like to do this 
- without the overhead of direct owner-to-owner negotiations
- without first liquidating the tokens to some common currency (e.g. ETH)

Instead we create a pool, where the owners are asked to each rank the tokens in the pool in order of their preferences. This ranking does not have to be total, i.e. you can select only the tokens you would be interested in trading for. The Top Trading Cycle algorithm will take all preferences into account and "optimally" re-allocate the tokens among the traders (see the algorithm details for definition of optimal). Furthermore, there is no incentive for the traders to do anything other than follow the mechanism "honestly"  (again see the details).

## Background
The [wikipedia article](https://en.wikipedia.org/wiki/Top_trading_cycle) does a good job explaining what the algorithm and setting is, and hints at various generalizations with links in the footnotes.

## Architecture
1. A Solidity smart contract capabale of 
    - holding NFTs in a custodial mannor (ideally with safe retrieval in case the participant wants to exit before completion)
    - accepting trading preferences
    - "locking down" for a period of time long enough to execute the trading algorithm off chain
    - accepting and validating proofs for the results of the trading algorithm (a "re-allocation")
    - allowing users to withdraw according to re-allocation
2. A rust implementation of the Top Trading Cycle (TTC) algorithm, and a compatibility layer for inputs/outputs expected by the contract.
3. Risc-Zero zkvm + [Steel](https://github.com/risc0/risc0-ethereum/tree/main/crates/steel) for generating Groth16 proofs of the TTC execution on smart contract data.
4. TODO: A server to monitor the contracts for proof requests / callbacks, and a gpu accelerated environment to construct the proofs.
5. TODO: A simple UI + testnet deployment for illustration purposes.

## Test against local node
The `host` crate contains an end-to-end test using a randomly generated allocation. See the [node_test](https://github.com/l-adic/ttc/blob/main/.github/workflows/node_test.yml) workflow for how you would set these up and run locally.

## Development Environment

### Development Environment Setup

Two tmuxinator configurations are provided for development:

#### 1. Local Development (`.tmuxinator.yml`)
Run services locally with cargo:
```bash
tmuxinator start -p .tmuxinator.yml
```

#### 2. Docker Development (`.tmuxinator.docker.yml`)
Run all services in Docker containers:
```bash
tmuxinator start -p .tmuxinator.docker.yml
```

The Docker configuration automatically rebuilds the prover-server and monitor-server images before starting to ensure they reflect your current code. This is important when you've made changes to:
- Any Rust source files
- Cargo.toml dependencies
- Dockerfile configurations

If you need to manually rebuild the images:
```bash
docker compose build prover-server monitor-server
```

Both configurations set up:
- Ethereum node and Postgres logs
- Prover and Monitor servers
- System monitoring (htop)
- Command shell

#### Tmux Key Bindings
- Exit/detach from tmux: `Ctrl-b d`
- Switch between windows: `Ctrl-b [0-9]` or mouse click
- Switch between panes: `Ctrl-b arrow` or mouse click
- Scroll in a pane: Mouse wheel or `Ctrl-b [` then arrow keys (press `q` to exit scroll mode)
- Copy text: Select with mouse (may need to hold Shift depending on your terminal)
