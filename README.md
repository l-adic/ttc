# Top Trading Cycle

## Overview
A group of people owning various ERC-721 (i.e non-fungible) tokens would like to trade amongst each other. They would like to do this 
- without the overhead of direct owner-to-owner negotiations
- without first liquidating the tokens to some common currency (e.g. ETH)

Instead we create a pool, where the owners are asked to each rank the tokens in the pool in order of their preferences. This ranking does not have to be total, i.e. you can select only the tokens you would be interested in trading for. The Top Trading Cycle algorithm will take all preferences into account and "optimally" re-allocate the tokens among the traders (see the algorithm details for definition of optimal). Furthermore, there is no incentive for the traders to do anything other than follow the mechanism "honestly"  (again see the details).

## Background
The [wikipedia article](https://en.wikipedia.org/wiki/Top_trading_cycle) does a good job explaining what the algorithm and setting is, and hints at various generalizations with links in the footnotes.

## Architecture
For architecture diagrams, see [here](./docs/README.md#architecture-diagram). For more details on the flows and service interaction, see [here](./docs/README.md#flows)

## Development Environment Setup

Two tmuxinator configurations are provided for development. Both configurations set up:
- Ethereum node and Postgres
- Prover and Monitor servers
- System monitoring (e.g htop, nvidia-smi)
- Command shell


#### 1. Local Development (`.tmuxinator.yml`)
Run services locally with cargo:
```bash
make build-monitor
make build-prover # Or make make build-prover-cuda for gpu
RISC0_DEV_MODE={true/false} tmuxinator start -p .tmuxinator.yml
```

#### 2. Docker Development (`.tmuxinator.docker.yml`)
make build-monitor
make build-prover # Or make make build-prover-cuda for gpu
Run all services in Docker containers:
```bash
RISC0_DEV_MODE={true/false} tmuxinator start -p .tmuxinator.docker.yml
```

## Testing
The `host` crate contains an end-to-end test using a randomly generated allocation. You can check the corresponding [github workflow](./.github/workflows/node_test.yml) for reference,
and view the [Makefile](./Makefile) for a complete set of config options

1. Deploy the services (see [above](./README.md#development-environment-setup)) or use a hosted deployment.

2. You must fetch the `ImageID.sol` contract and store it in the correct location. This contract is what cryptographically binds the solidity verifier to the rust TTC program.

```bash
> make fetch-image-id-contract > contract/src/ImageID.sol
```

3. Deploy the contracts:

```
> make deploy
```
NOTE: use the `deploy-mock` variant if you are running a mock verifier (recommended if you aren't using a cuda accelerated prover).
You should see the deploy address of the TTC contract printed to the console. There will be a json artifact written to `./deployments/<ttc-contract-address>/deployed.json`

4. Run the demo script with the desired config options, e.g.

```
> TTC_ADDRESS=<ttc-contract-address> NUM_ACTORS=10 PROVER_TIMEOUT=600 make run-node-tests
```

You can control the log level via `RUST_LOG`. This script creates checkpoints, writing the relevant state to `./deployments/<ttc-contract-address>`. If the script errors or halts at any
time, you can re-run from the last checkpoint using the same command as you used to start.
