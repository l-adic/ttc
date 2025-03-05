# Use a Rust base image with the specific version from rust-toolchain.toml
FROM rust:1.85.0-slim-bullseye

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install RISC0 toolchain
RUN curl -L https://risczero.com/install | bash
ENV PATH="/root/.risc0/bin:${PATH}"
RUN /root/.risc0/bin/rzup install
RUN /root/.risc0/bin/rzup install cargo-risczero 1.2.4
RUN /root/.risc0/bin/rzup install r0vm 1.2.4

# Install Foundry
RUN curl -L https://foundry.paradigm.xyz | bash
RUN /root/.foundry/bin/foundryup
ENV PATH="/root/.foundry/bin:${PATH}"


# Set the working directory
WORKDIR /app

# Copy the entire repository
COPY . .

# Build the prover-server binary
RUN RUST_BACKTRACE=1 make build-prover

# Set the binary as the default command
CMD ["/app/target/release/prover-server"]
