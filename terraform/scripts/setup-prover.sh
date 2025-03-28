#!/bin/sh

export RUST_LOG="${rust_log_level}"
export RISC0_DEV_MODE="${risc0_dev_mode}"
export DB_HOST="${db_host}"
export DB_USER="${db_user}"
export DB_PASSWORD="${db_password}"
export DB_NAME="${db_name}"
export DB_PORT="5432"
export NODE_HOST="${node_host}"
export NODE_PORT="8545"
export JSON_RPC_PORT="3000"

# Initialize NVIDIA GPU
yes y | /opt/nvidia/gcp-ngc-login.sh true none false /var/log/nvidia 2> /dev/null

# Then echo the actual exported environment variables to confirm they're set correctly
echo "=== TTC Prover Environment Variables After Export ==="
echo "RUST_LOG=$RUST_LOG"
echo "RISC0_DEV_MODE=$RISC0_DEV_MODE"
echo "DB_HOST=$DB_HOST"
echo "DB_USER=$DB_USER"
echo "DB_PASSWORD=[REDACTED]"
echo "DB_NAME=$DB_NAME"
echo "DB_PORT=$DB_PORT"
echo "NODE_HOST=$NODE_HOST"
echo "NODE_PORT=$NODE_PORT"
echo "JSON_RPC_PORT=$JSON_RPC_PORT"


# Ensure non-interactive installation
export DEBIAN_FRONTEND=noninteractive

# Install system dependencies
apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    curl \
    wget

# Install CUDA toolkit and drivers following NVIDIA's instructions
wget https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/cuda-keyring_1.1-1_all.deb
dpkg -i cuda-keyring_1.1-1_all.deb
apt-get update
apt-get -y install --no-install-recommends cuda-toolkit-12-4
apt-get -y install --no-install-recommends cuda-drivers

# Set CUDA environment variables
echo "export PATH=/usr/local/cuda/bin:$${PATH}" >> /etc/environment
echo "export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$${LD_LIBRARY_PATH}" >> /etc/environment
echo "export CUDA_VISIBLE_DEVICES=all" >> /etc/environment
. /etc/environment

# Create build user and directory
groupadd builder
useradd -m -g builder -s /bin/bash builder
usermod -a -G video builder
mkdir -p /home/builder/code
chown -R builder:builder /home/builder/code

# Add environment variables to builder's profile
su - builder -c "bash -c 'echo export PATH=/usr/local/cuda/bin:/usr/local/bin:\$PATH >> ~/.profile'"
su - builder -c "bash -c 'echo export LD_LIBRARY_PATH=/usr/local/cuda/lib64:\$LD_LIBRARY_PATH >> ~/.profile'"
su - builder -c "bash -c 'echo export CUDA_VISIBLE_DEVICES=all >> ~/.profile'"

# Install Rust and source env
su - builder -c "bash -c 'curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain stable'"
su - builder -c "bash -c 'echo . \$HOME/.cargo/env >> ~/.profile'"
su - builder -c "bash -c 'source ~/.profile && rustc --version'"

# Install RISC0 after Rust is ready
su - builder -c "bash -c 'source ~/.profile && curl -L https://risczero.com/install | bash'"
su - builder -c "bash -c 'echo export PATH=/home/builder/.risc0/bin:\$PATH >> ~/.profile'"
su - builder -c "bash -c 'source ~/.profile && source ~/.bashrc && rzup install'"
su - builder -c "bash -c 'source ~/.profile && source ~/.bashrc && rzup install'"
su - builder -c "bash -c 'source ~/.profile && source ~/.bashrc && rzup install cargo-risczero 1.2.4'"
su - builder -c "bash -c 'source ~/.profile && source ~/.bashrc && rzup install r0vm 1.2.4'"

# Copy RISC0 tools to system-wide location
cp /home/builder/.risc0/bin/* /usr/local/bin/
chmod +x /usr/local/bin/*


# Verifty rzup is available
su - builder -c "bash -c 'source ~/.profile && source ~/.bashrc && rzup --version'" || { echo "rzup not found in PATH"; exit 1; }

# Install Foundry
su - builder -c "bash -c 'curl -L https://foundry.paradigm.xyz | bash -s -- -y'"
sleep 2  # Wait for foundry installation to complete
su - builder -c "bash -c 'test -d ~/.foundry || mkdir -p ~/.foundry/bin'"
su - builder -c "bash -c 'test -f ~/.foundry/bin/foundryup || curl -L https://raw.githubusercontent.com/foundry-rs/foundry/master/foundryup/foundryup -o ~/.foundry/bin/foundryup'"
su - builder -c "bash -c 'chmod +x ~/.foundry/bin/foundryup'"
su - builder -c "bash -c 'echo export PATH=/home/builder/.foundry/bin:\$PATH >> ~/.profile'"
su - builder -c "bash -c 'source ~/.profile'"  # Source profile to get foundry in PATH
su - builder -c "bash -c 'source ~/.bashrc'"
su - builder -c "bash -c 'foundryup --install 0.3.0'"

# Copy foundry binaries to system-wide location immediately after installation
cp /home/builder/.foundry/bin/* /usr/local/bin/
chmod +x /usr/local/bin/*

# Verify forge is available
su - builder -c "bash -c 'source ~/.profile && source ~/.bashrc && forge --version'" || { echo "forge not found in PATH"; exit 1; }

# Clone and build as non-root
rm -rf /home/builder/code/ttc
su - builder -c "bash -c 'cd /home/builder/code && \
    git clone https://github.com/l-adic/ttc.git && \
    cd ttc && \
    source ~/.profile && \
    source ~/.bashrc && \
    make compile-contract-deps && \
    RUSTFLAGS=\"-C target-cpu=native\" \
    RUST_BACKTRACE=1 \
    make build-prover-cuda'"

if [ ! -f "/home/builder/code/ttc/target/release/prover-server" ]; then
    echo "Binary not found at expected location"
    exit 1
fi

# Copy binary to system location
mkdir -p /opt/ttc/target/release
cp /home/builder/code/ttc/target/release/prover-server /opt/ttc/target/release/
cp -r /home/builder/code/ttc/monitor/contract /opt/ttc/monitor/

# Create work directory
mkdir -p /tmp/risc0-work-dir

# Create systemd service
cat > /etc/systemd/system/prover.service <<SERVICEEOF
[Unit]
Description=TTC Prover Server
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/opt/ttc
Environment="PATH=/usr/local/cuda/bin:/usr/local/bin:$${PATH}"
Environment="LD_LIBRARY_PATH=/usr/local/cuda/lib64:$${LD_LIBRARY_PATH}"
Environment="CUDA_VISIBLE_DEVICES=all"
Environment="RUST_LOG=$${RUST_LOG}"
Environment="RISC0_DEV_MODE=$${RISC0_DEV_MODE}"
Environment="DB_HOST=$${DB_HOST}"
Environment="DB_USER=$${DB_USER}"
Environment="DB_PASSWORD=$${DB_PASSWORD}"
Environment="DB_NAME=$${DB_NAME}"
Environment="DB_PORT=$${DB_PORT}"
Environment="NODE_HOST=$${NODE_HOST}"
Environment="NODE_PORT=$${NODE_PORT}"
Environment="JSON_RPC_PORT=$${JSON_RPC_PORT}"
Environment="RISC0_PROVER=local"
Environment="RUST_BACKTRACE=1"
Environment="RISC0_WORK_DIR=/tmp/risc0-work-dir"
Environment="IMAGE_ID_CONTRACT=/opt/ttc/monitor/ImageID.sol"

ExecStart=/opt/ttc/target/release/prover-server
WorkingDirectory=/opt/ttc
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
SERVICEEOF

# Start the service
systemctl daemon-reload
systemctl enable prover.service
systemctl start prover.service