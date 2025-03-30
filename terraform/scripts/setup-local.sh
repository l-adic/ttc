# nvidia drivers and cuda-toolkit@12.8
sudo apt-get update
sudo apt install -y ubuntu-drivers-common
sudo ubuntu-drivers install
sudo apt install -y build-essential libssl-dev -y

wget https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/cuda-keyring_1.1-1_all.deb
sudo dpkg -i cuda-keyring_1.1-1_all.deb
sudo apt-get update
sudo apt-get -y install cuda-toolkit-12-8

echo 'export PATH=/usr/local/cuda/bin:$PATH' >> ~/.bashrc
echo 'export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$LD_LIBRARY_PATH' >> ~/.bashrc

# risc0 build tools
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain stable
. $HOME/.cargo/env
source ~/.bashrc
rustc --version  || { echo "rustc not found in PATH"; exit 1; }

curl -L https://risczero.com/install | bash
source ~/.bashrc
rzup --version  || { echo "rzup not found in PATH"; exit 1; }
rzup install
rzup install
rzup install cargo-risczero 1.2.4
rzup install r0vm 1.2.4


curl -L https://foundry.paradigm.xyz | bash -s -- -y
source ~/.bashrc
foundryup --install 0.3.0
forge --version || { echo "forge not found in PATH"; exit 1; }

# build project
source ~/.bashrc
git clone https://github.com/l-adic/ttc.git
cd ttc
make compile-contract-deps
RUSTFLAGS="-C target-cpu=native" RUST_BACKTRACE=1 make build-prover-cuda