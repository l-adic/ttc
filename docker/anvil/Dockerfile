# Build stage
FROM ubuntu:22.04 as builder

# Install minimal dependencies
RUN apt-get update && apt-get install -y curl git

# Install Foundry and extract just the anvil binary
RUN curl -L https://foundry.paradigm.xyz | bash && \
    /root/.foundry/bin/foundryup && \
    mkdir -p /out && \
    cp /root/.foundry/bin/anvil /out/

# Runtime stage
FROM debian:bullseye-slim

# Copy only the anvil binary
COPY --from=builder /out/anvil /usr/local/bin/

# Expose the default Anvil port
EXPOSE 8545

# Default command to run Anvil
CMD ["anvil"]
