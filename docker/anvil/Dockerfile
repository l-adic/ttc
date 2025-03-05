FROM ubuntu:22.04

# Install dependencies
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    git \
    && rm -rf /var/lib/apt/lists/*

# Install Foundry
RUN curl -L https://foundry.paradigm.xyz | bash
RUN /root/.foundry/bin/foundryup

# Add foundry binaries to PATH
ENV PATH="/root/.foundry/bin:${PATH}"

# Expose the default Anvil port
EXPOSE 8545

# Set the working directory
WORKDIR /app

# Default command to run Anvil
CMD ["anvil"]
