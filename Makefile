.PHONY: build-methods build-contracts build test clean lint fmt check all run-node-tests run-node-tests-mock help

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

# Node tests
run-node-tests: build ## Run node tests with real RISC0
	RUST_LOG=info cargo run -p host --release -- \
		--max-actors 3 \
		--chain-id 31337 \
		--owner-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

run-node-tests-mock: build ## Run node tests with mocked RISC0
	RUST_LOG=info RISC0_DEV_MODE=true cargo run -p host --release -- \
		--max-actors 20 \
		--chain-id 31337 \
		--owner-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
