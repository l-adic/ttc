# Terraform Configuration for TTC Project

This Terraform configuration deploys the TTC project infrastructure on Google Cloud Platform.

## Architecture

- **Anvil Node**: Preemptible VM running Anvil (Foundry's local Ethereum node)
  - 2 vCPU, 8GB RAM
  - Auto-restart on preemption
  - Internal load balancer
  - Docker image: `elladic/anvil-node:<tag>`

- **Prover Server**: Cloud Run service
  - 16 vCPU, 32GB RAM
  - Scale to zero enabled
  - Internal access only
  - Docker image: `elladic/ttc-prover-server:<tag>`

- **Networking**:
  - Private VPC network
  - Cloud NAT for outbound internet
  - IAP for secure access
  - Internal load balancers

## Prerequisites

1. Install required tools:
   - [Terraform](https://www.terraform.io/downloads.html) (>= 1.0.0)
   - [Google Cloud SDK](https://cloud.google.com/sdk/docs/install)

2. Configure Google Cloud SDK:
   ```bash
   gcloud auth login
   gcloud auth application-default login
   ```

## Configuration

Create a `terraform.tfvars` file with your specific configuration:

```hcl
# Google Cloud Platform Configuration
gcp_project_id = "your-project-id"
gcp_region     = "us-central1"
gcp_zone       = "us-central1-a"

# Network Configuration
vpc_network_name = "ttc-network"

# Docker Image Configuration
docker_image_tag = "latest"  # or specific version tag

# Anvil Node Configuration
anvil_machine_type     = "e2-standard-2"
anvil_chain_id        = 31337
anvil_account_count   = 10
anvil_account_balance = 100000

# Prover Server Configuration
prover_cpu_count       = 16
prover_memory_gb       = 32
prover_rust_log_level  = "info"
prover_risc0_dev_mode  = "true"

# IAP Configuration
iap_member_list = [
  "user:your-email@example.com"
]
```

## Deployment

1. Initialize Terraform:
   ```bash
   terraform init
   ```

2. Review the plan:
   ```bash
   terraform plan
   ```

3. Apply the configuration:
   ```bash
   terraform apply
   ```

## Accessing Services

After deployment, Terraform will output instructions for accessing the services. You'll need to:

1. Create IAP tunnels to access the services:
   ```bash
   # Terminal 1: Anvil Node
   gcloud compute start-iap-tunnel ethereum-node 8545 --local-host-port=localhost:8545

   # Terminal 2: Prover Server
   gcloud compute start-iap-tunnel prover-server 8546 --local-host-port=localhost:8546
   ```

2. Services will be available at:
   - Anvil Node: `http://localhost:8545`
   - Prover Server: `http://localhost:8546`

## Cost Optimization

- The Anvil node uses a preemptible VM (~70% cheaper)
- The Prover server scales to zero when not in use
- All networking is internal to minimize costs

## Maintenance

- The Anvil node will automatically restart if preempted
- Cloud Run service updates can be triggered by pushing new Docker images
- Infrastructure changes should be made through Terraform

## Cleanup

To destroy all resources:
```bash
terraform destroy
```

## Security Notes

- All services are internal-only
- Access is controlled via IAP
- Service accounts use minimal permissions
- No public IP addresses are exposed
