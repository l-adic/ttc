# Build stage
FROM rust:1.85.0-slim-bullseye as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    curl

# Install RISC0 toolchain
RUN curl -L https://risczero.com/install | bash && \
    /root/.risc0/bin/rzup install && \
    /root/.risc0/bin/rzup install cargo-risczero 1.2.4 && \
    /root/.risc0/bin/rzup install r0vm 1.2.4
ENV PATH="/root/.risc0/bin:${PATH}"

# Install Foundry and build contracts
RUN curl -L https://foundry.paradigm.xyz | bash && \
    /root/.foundry/bin/foundryup
ENV PATH="/root/.foundry/bin:${PATH}"

# Create a new empty project for building prover-server
WORKDIR /app
COPY . .

# Build the prover-server binary
RUN make build-prover


# Runtime stage
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl1.1 \
    ca-certificates

# Create app directory structure
WORKDIR /app

COPY --from=builder /usr/local/cargo/bin/r0vm /usr/local/bin/r0vm
COPY --from=builder /app/target/release/prover-server /usr/local/bin/prover-server

# Set the binary as the entrypoint
CMD ["prover-server"]
