.PHONY: build test clean lint fmt check all help

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
	

# Build commands
build:
	cd contract && forge build
	cargo build --release --workspace

# Test commands
test:
	cargo test --release --workspace

# Cleaning
clean:
	cargo clean

# Linting
lint:
	cargo clippy --workspace -- -D warnings

# Formatting
fmt:
	cargo fmt --all

# Check everything
check: fmt lint build test
