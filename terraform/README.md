# Terraform Configuration for TTC Project

This Terraform configuration deploys the TTC project infrastructure on Google Cloud Platform.

## Architecture

- **Cloud SQL**: PostgreSQL database instance
  - Private IP access only
  - Automatic backups enabled
  - Connected to VPC network
  - Used by both Monitor and Prover servers

- **Anvil Node**: Preemptible VM running Anvil (Foundry's local Ethereum node)
  - 2 vCPU, 8GB RAM
  - Auto-restart on preemption
  - Internal load balancer
  - Docker image: `elladic/anvil-node:<tag>`

- **Monitor Server**: Compute Engine VM
  - 2 vCPU, 8GB RAM
  - Always running
  - Internal load balancer
  - Docker image: `elladic/ttc-monitor-server:<tag>`

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

2. Configure Google Cloud SDK and permissions:
   ```bash
   # Login to Google Cloud
   gcloud auth login
   gcloud auth application-default login

   # Grant yourself necessary roles (if you have owner access):
   gcloud projects add-iam-policy-binding YOUR_PROJECT_ID \
     --member="user:YOUR_EMAIL" \
     --role="roles/cloudsql.admin"

   gcloud projects add-iam-policy-binding YOUR_PROJECT_ID \
     --member="user:YOUR_EMAIL" \
     --role="roles/compute.networkAdmin"

   gcloud projects add-iam-policy-binding YOUR_PROJECT_ID \
     --member="user:YOUR_EMAIL" \
     --role="roles/servicenetworking.networksAdmin"
   ```

   Required roles:
   - Cloud SQL Admin (roles/cloudsql.admin)
   - Compute Network Admin (roles/compute.networkAdmin)
   - Service Networking Admin (roles/servicenetworking.networksAdmin)
   - Or Project Owner role which includes these

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

# Cloud SQL Configuration
database_instance_name = "ttc-postgres"
database_version      = "POSTGRES_15"
database_tier         = "db-f1-micro"  # Adjust based on needs
database_name         = "ttc"
database_username     = "ttc_user"
database_password     = "your-secure-password-here"
database_deletion_protection = false    # Set to true in production
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

4. Run database migrations:
   ```bash
   cd terraform
   ./scripts/migrate.sh "your-database-password"
   ```
   IMPORTANT: Migrations must be run before starting the services. The services expect the database and schema to already exist.

### Targeted Deployments

For faster iterations during development, you can target specific resources:

1. **Redeploy Monitor Server Only**:
   ```bash
   # Update just the monitor server
   terraform apply -target=google_compute_instance_template.monitor_server

   # If you also need to update its service account
   terraform apply -target=google_service_account.monitor_server -target=google_compute_instance_template.monitor_server
   ```

2. **Redeploy Prover Server Only**:
   ```bash
   # Update just the prover server
   terraform apply -target=google_cloud_run_v2_service.prover_server

   # If you also need to update its service account
   terraform apply -target=google_service_account.prover_server -target=google_cloud_run_v2_service.prover_server
   ```

3. **Force a Redeploy** (if you just want to restart with the same configuration):
   ```bash
   # For monitor server
   terraform taint google_compute_instance_group_manager.monitor_server
   terraform apply -target=google_compute_instance_group_manager.monitor_server

   # For prover server
   terraform taint google_cloud_run_v2_service.prover_server
   terraform apply -target=google_cloud_run_v2_service.prover_server
   ```

4. **Update Docker Image Only**:
   ```bash
   # For monitor server
   terraform apply -var="docker_image_tag=v1.0.1" -target=google_compute_instance_template.monitor_server

   # For prover server
   terraform apply -var="docker_image_tag=v1.0.1" -target=google_cloud_run_v2_service.prover_server
   ```

### Database Management

1. **Initial Setup**:
   - Cloud SQL instance creation typically takes 10-15 minutes
   - This is normal and expected due to:
     - VM provisioning
     - Network setup (VPC peering)
     - Storage allocation
     - Database initialization
   - You can monitor progress in:
     - GCP Console: https://console.cloud.google.com/sql/instances
     - CLI: `gcloud sql operations list --instance=ttc-postgres`

2. **Database Migrations**:
   - Migrations are handled separately from the services
   - Must be run after infrastructure is created but before services start
   - Run using the provided script:
     ```bash
     cd terraform
     ./scripts/migrate.sh "your-database-password"
     ```
   - The script will:
     - Create the database if it doesn't exist
     - Apply the schema migrations
     - Handle all necessary setup

3. **Development Strategies**:
   - For local development, use the provided docker-compose.yml which includes a PostgreSQL container
   - Keep a stable Cloud SQL instance for development/staging to avoid frequent recreation
   - Use terraform workspaces or separate state files for dev/prod environments

## Accessing Services

After deployment, Terraform will output instructions for accessing the services. You'll need to:

1. Create IAP tunnels to access the services:
   ```bash
   # Terminal 1: Anvil Node
   gcloud compute start-iap-tunnel ethereum-node-xxxx 8545 --local-host-port=localhost:8545 --zone=<your-zone>

   # Terminal 2: Monitor Server
   gcloud compute start-iap-tunnel monitor-server-xxxx 3030 --local-host-port=localhost:3030 --zone=<your-zone>
   ```
   Note: Replace xxxx with the actual instance name from `gcloud compute instances list`

2. Services will be available at:
   - Anvil Node: `http://localhost:8545`
   - Monitor Server: `http://localhost:3030`

Note: The Prover Server is only accessible internally to the Monitor Server and cannot be accessed directly.

## Cost Optimization

- The Anvil node uses a preemptible VM (~70% cheaper)
- The Prover server scales to zero when not in use
- All networking is internal to minimize costs
- Cloud SQL uses minimal instance type by default (db-f1-micro)
- Consider using local PostgreSQL for development to avoid cloud costs

## Maintenance

- The Anvil node will automatically restart if preempted
- Cloud Run service updates can be triggered by pushing new Docker images
- Infrastructure changes should be made through Terraform

## Troubleshooting

1. **Cloud Run Service Not Starting**:
   - Check container logs in Cloud Console
   - Verify port configuration matches (default: 8546 for prover)
   - Check environment variables are correct
   - Use `gcloud run services describe prover-server` for status

2. **Database Creation Taking Too Long**:
   - Check operation status: `gcloud sql operations list --instance=ttc-postgres`
   - View detailed logs in Cloud Console SQL section
   - Normal creation time is 10-15 minutes
   - If >20 minutes, check for:
     - Network connectivity issues
     - Resource quota limits
     - Service API errors

3. **Permission Issues**:
   - Ensure you have required roles (see Prerequisites)
   - Check service account permissions
   - Review IAM audit logs in Cloud Console

## Cleanup

To destroy all resources:

1. If terraform destroy fails with VPC network in use:
   ```bash
   # Remove resources from state
   terraform state rm google_service_networking_connection.private_vpc_connection
   terraform state rm google_compute_global_address.private_ip_address
   terraform state rm google_compute_network.vpc

   # Delete Cloud SQL instance
   gcloud sql instances delete ttc-postgres --quiet

   # Then try destroy again
   terraform destroy
   ```

2. For clean redeployment:
   ```bash
   terraform apply
   ```
   The resources will be created in the correct order:
   - VPC network
   - Private IP allocation
   - VPC peering connection
   - Cloud SQL instance
   - Database users and schema

Note: The database user (ttc_user) is configured with deletion_policy = "ABANDON" to prevent errors during cleanup. The postgres superuser is configured as BUILT_IN for proper permissions.

## Security Notes

- All services are internal-only
- Access is controlled via IAP
- Service accounts use minimal permissions
- No public IP addresses are exposed
