# Use NVIDIA CUDA base image with x86 architecture
FROM nvidia/cuda:12.4.0-devel-ubuntu22.04 AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    curl \
    ubuntu-drivers-common \
    && rm -rf /var/lib/apt/lists/*

# Set CUDA environment variables
ENV PATH=/usr/local/cuda/bin:${PATH}
ENV LD_LIBRARY_PATH=/usr/local/cuda/lib64:${LD_LIBRARY_PATH}
ENV CUDA_VISIBLE_DEVICES=all

# Note: This image requires NVIDIA GPU with CUDA support
# When running, use: docker run --gpus all prover-server-cuda

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

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

RUN RUST_BACKTRACE=1 make build-prover-cuda

FROM nvidia/cuda:12.4.0-runtime-ubuntu22.04 AS cuda-prover-server

RUN apt-get update && \
    apt-get -qy full-upgrade && \
    apt-get install -qy \
    libssl3 \
    ca-certificates \
    libc6 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# The Groth16 Receipt generation is run in docker
RUN curl -sSL https://get.docker.com/ | sh

# Create the risc0 work directory (this is a shared volume for the groth16 work)
RUN mkdir -p /tmp/risc0-work-dir
    
COPY --from=builder /app/target/release/prover-server /app/target/release/

# Set the binary as the entrypoint
ENTRYPOINT ["/app/target/release/prover-server"]
