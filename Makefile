# Default environment variables
NODE_HOST ?= localhost
NODE_PORT ?= 8545
MONITOR_HOST ?= localhost
MONITOR_PORT ?= 3030
PROVER_HOST ?= localhost
PROVER_PORT ?= 3000

# Database defaults
DB_HOST ?= localhost
DB_PORT ?= 5432
DB_USER ?= postgres
DB_PASSWORD ?= postgres
DB_NAME ?= ttc

.PHONY: build-methods build-contracts build-prover build-host build test clean \
	lint fmt check all run-prover-server run-monitor-server \
	run-node-tests run-node-tests-mock create-db create-schema help

.DEFAULT_GOAL := help

# Default target
all: build test

# Generate help automatically from comments
help:
	@echo "Available commands:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  make %-20s - %s\n", $$1, $$2}'

# Build commands
build-methods: ## Build the RISC Zero guest program
	cargo build -p methods --release

build-contracts: build-methods ## Build smart contracts (requires guest)
	cd contract && forge build

build-prover: build-contracts
	cargo build -p prover-server --release

build-monitor: build-contracts
	cargo build -p monitor-server --release

build-host: build-contracts ## Build the RISC Zero host program
	cargo build -p host --release

build: build-contracts ## Build all components (guests, contracts, host)
	cargo build --release --workspace

# Test commands
test: build-contracts ## Run all test suites (excluding integration tests)
	cargo test --release --workspace --exclude-ignored

test-integration: ## Run integration tests that require external services
	DB_HOST=$(DB_HOST) \
	DB_PORT=$(DB_PORT) \
	DB_USER=$(DB_USER) \
	DB_PASSWORD=$(DB_PASSWORD) \
	DB_NAME=$(DB_NAME) \
	RUST_LOG=debug \
	cargo test --release --workspace -- --ignored

# Cleaning
clean: ## Clean build artifacts
	cargo clean

# Linting and formatting
lint: ## Run code linters
	RISC0_SKIP_BUILD=1 cargo clippy --workspace -- -D warnings

fmt: ## Format code
	cargo fmt --all

# Check everything
check: fmt lint build test ## Run all checks (format, lint, build, test)

# Node tests
CHAIN_ID ?= 31337
OWNER_KEY ?= 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
MOCK_VERIFIER ?= false
RISC0_DEV_MODE ?= true

run-node-tests-mock: ## Run node tests with mock verifier
	RUST_LOG=info \
	NODE_HOST=$(NODE_HOST) \
	NODE_PORT=$(NODE_PORT) \
	MONITOR_HOST=$(MONITOR_HOST) \
	MONITOR_PORT=$(MONITOR_PORT) \
	MAX_ACTORS=20 \
	cargo run --bin host --release -- \
		--chain-id $(CHAIN_ID) \
		--owner-key $(OWNER_KEY) \
		--mock-verifier

run-node-tests: ## Run node tests with real verifier
	RUST_LOG=info \
	NODE_HOST=$(NODE_HOST) \
	NODE_PORT=$(NODE_PORT) \
	MONITOR_HOST=$(MONITOR_HOST) \
	MONITOR_PORT=$(MONITOR_PORT) \
	MAX_ACTORS=3 \
	cargo run --bin host --release -- \
		--chain-id $(CHAIN_ID) \
		--owner-key $(OWNER_KEY)

create-db: ## Create the database
	DB_HOST=$(DB_HOST) \
	DB_PORT=$(DB_PORT) \
	DB_USER=$(DB_USER) \
	DB_PASSWORD=$(DB_PASSWORD) \
	DB_NAME=postgres \
	DB_CREATE_NAME=ttc \
	RUST_LOG=debug \
	cargo run --release --bin create-db

create-schema: ## Create the database schema (Must setup the database first via create-db)
	DB_HOST=$(DB_HOST) \
	DB_PORT=$(DB_PORT) \
	DB_USER=$(DB_USER) \
	DB_PASSWORD=$(DB_PASSWORD) \
	DB_NAME=$(DB_NAME) \
	RUST_LOG=debug \
	cargo run --release --bin create-schema

run-prover-server: build-prover ## Run the prover server
	RUST_LOG=info \
	DB_HOST=$(DB_HOST) \
	DB_PORT=$(DB_PORT) \
	DB_USER=$(DB_USER) \
	DB_PASSWORD=$(DB_PASSWORD) \
	DB_NAME=$(DB_NAME) \
	NODE_HOST=$(NODE_HOST) \
	NODE_PORT=$(NODE_PORT) \
	JSON_RPC_PORT=$(PROVER_PORT) \
    RISC0_DEV_MODE=${RISC0_DEV_MODE} \
	cargo run --bin prover-server --release

run-monitor-server: build-monitor ## Run the monitor server
	RUST_LOG=info \
	DB_HOST=$(DB_HOST) \
	DB_PORT=$(DB_PORT) \
	DB_USER=$(DB_USER) \
	DB_PASSWORD=$(DB_PASSWORD) \
	DB_NAME=$(DB_NAME) \
	NODE_HOST=$(NODE_HOST) \
	NODE_PORT=$(NODE_PORT) \
	PROVER_HOST=$(PROVER_HOST) \
	PROVER_PORT=$(PROVER_PORT) \
	JSON_RPC_PORT=$(MONITOR_PORT) \
	cargo run --bin monitor-server --release
