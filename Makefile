.PHONY: build-methods build-contracts build-prover build test clean lint fmt check all run-proving-server run-mock-proving-server run-node-tests run-node-tests-mock help

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

build-prover: build-methods
	cargo build -p prover-server --release

build: build-contracts ## Build all components (guests, contracts, host)
	cargo build --release --workspace

# Test commands
test: build-contracts ## Run all test suites
	cargo test --release --workspace

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

run-mock-proving-server: build-prover ## Start proving server
	RUST_LOG=info RISC0_DEV_MODE=true cargo run --release --bin prover-server

# Node tests
run-node-tests-mock: build ## Run node tests with real RISC0
	RUST_LOG=info cargo run --bin host --release -- \
		--max-actors 3 \
		--chain-id 31337 \
		--owner-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
		--mock-verifier

run-proving-server: build-prover ## Start proving server
	RUST_LOG=info cargo run --release --bin prover-server

run-node-tests: build ## Run node tests with real RISC0
	RUST_LOG=info cargo run --bin host --release -- \
		--max-actors 3 \
		--chain-id 31337 \
		--owner-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80