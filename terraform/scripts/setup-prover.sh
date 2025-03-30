#!/bin/bash
set -e

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


# Install NVIDIA drivers and CUDA toolkit
apt-get update
apt install -y ubuntu-drivers-common
ubuntu-drivers install
apt install -y build-essential libssl-dev
wget https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/cuda-keyring_1.1-1_all.deb
dpkg -i cuda-keyring_1.1-1_all.deb
apt-get update
apt-get -y install cuda-toolkit-12-8

export PATH=/usr/local/cuda/bin:$PATH
export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$LD_LIBRARY_PATH

export HOME=/root

# Install Rust globally
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
# Add cargo to PATH for root
export PATH="/root/.cargo/bin:$PATH"

# Verify rustc
rustc --version || { echo "rustc not found in PATH"; exit 1; }

# Install RISC Zero
curl -L https://risczero.com/install | bash
export PATH="/root/.risc0/bin:$PATH"

# Verify rzup
rzup --version || { echo "rzup not found in PATH"; exit 1; }

# Install risc zero components
rzup install
rzup install cargo-risczero 1.2.4
rzup install r0vm 1.2.4

# Install Foundry
curl -L https://foundry.paradigm.xyz | bash -s -- -y
export PATH="/root/.foundry/bin:$PATH"

# Install specific foundry version
foundryup --install 0.3.0

# Verify forge
forge --version || { echo "forge not found in PATH"; exit 1; }

# Clone project (if it doesn't exist)
if [ ! -d "/opt/ttc" ]; then
  git clone https://github.com/l-adic/ttc.git /opt/ttc
fi


cd /opt/ttc
make compile-contract-deps
RUSTFLAGS="-C target-cpu=native" RUST_BACKTRACE=1 make build-prover-cuda

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
Environment="PATH=/usr/local/bin:$${PATH}"
Environment="LD_LIBRARY_PATH=$${LD_LIBRARY_PATH}"
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
Environment="IMAGE_ID_CONTRACT=/opt/ttc/monitor/contract/ImageID.sol"

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