.PHONY: build-guest build-contracts build test clean lint fmt check all run-node-tests run-node-tests-mock help

# Default target
all: build test

# Help command
help:
	@echo "Available commands:"
	@echo "  make build           - Build all Rust crates"
	@echo "  make test            - Run all tests"
	@echo "  make test-contracts  - Build and test contracts against a local node"
	@echo "  make clean           - Clean build artifacts"
	@echo "  make lint            - Run linters"
	@echo "  make fmt             - Format code"
	@echo "  make check           - Run all checks (build, test, lint)"
	@echo "  make all             - Build and test everything"

build-guest:
	cd methods/guest && cargo build -p ttc-guests --release

build-contracts: build-guest
	cargo build -p ttc-methods --release
	cd contract && forge build

# Build commands
build: build-contracts
	cargo build --release --workspace

# Test commands
test:
	cargo test --release --workspace

# Cleaning
clean:
	cargo clean

# Linting
lint:
	RISC0_SKIP_BUILD=1 cargo clippy --workspace -- -D warnings

# Formatting
fmt:
	cargo fmt --all

# Check everything
check: fmt lint build test

run-node-tests: build
	RUST_LOG=info RUST_BACKTRACE=1 cargo run -p host --release -- --max-actors 3 --chain-id 31337 --owner-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

run-node-tests-mock: build
	RUST_LOG=info RUST_BACKTRACE=1 cargo run -p host --release -- --mock --max-actors 3 --chain-id 31337 --owner-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
