# Shared base image with Rust toolchain and dependencies
FROM ubuntu:24.04 AS builder

# Install system dependencies and Rust
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.85.0

ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup component add rustfmt clippy

# Install RISC0 toolchain
RUN curl -L https://risczero.com/install | bash
ENV PATH="/root/.risc0/bin:${PATH}"
RUN /root/.risc0/bin/rzup install
RUN /root/.risc0/bin/rzup install cargo-risczero 1.2.4
RUN /root/.risc0/bin/rzup install r0vm 1.2.4

# Install Foundry
RUN curl -L https://foundry.paradigm.xyz | bash
RUN /root/.foundry/bin/foundryup --install 0.3.0 
ENV PATH="/root/.foundry/bin:${PATH}"

# Set the working directory
WORKDIR /app

COPY . .

RUN make build-servers

# Create monitor-server image
FROM ubuntu:24.04 AS monitor-server
RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/monitor-server /app/target/release/
CMD ["/app/target/release/monitor-server"]

# Create prover-server image
FROM ubuntu:24.04 AS prover-server
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    libc6 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && rm -rf /var/lib/apt/lists/* \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.85.0

ENV PATH="/root/.cargo/bin:${PATH}"

# Install RISC0 toolchain
RUN curl -L https://risczero.com/install | bash
ENV PATH="/root/.risc0/bin:${PATH}"
RUN /root/.risc0/bin/rzup install
RUN /root/.risc0/bin/rzup install cargo-risczero 1.2.4
RUN /root/.risc0/bin/rzup install r0vm 1.2.4

COPY --from=builder /app/target/release/prover-server /app/target/release/

CMD ["/app/target/release/prover-server"]
