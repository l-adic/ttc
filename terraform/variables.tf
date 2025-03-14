# Google Cloud Platform Configuration
variable "gcp_project_id" {
  description = "The GCP project ID"
  type        = string
}

variable "gcp_region" {
  description = "The GCP region for resource deployment"
  type        = string
  default     = "us-central1"
}

variable "gcp_zone" {
  description = "The GCP zone for resource deployment"
  type        = string
  default     = "us-central1-a"
}

# Network Configuration
variable "vpc_network_name" {
  description = "Name of the VPC network"
  type        = string
  default     = "ttc-network"
}

# Docker Image Configuration
variable "docker_image_tag" {
  description = "Tag for Docker images (e.g., latest, v1.0.0)"
  type        = string
}

# Anvil Node Configuration
variable "anvil_image_repository" {
  description = "Docker image repository for Anvil node"
  type        = string
  default     = "elladic/anvil-node"
}

variable "anvil_machine_type" {
  description = "GCP machine type for Anvil node"
  type        = string
  default     = "e2-standard-2"
}

variable "anvil_chain_id" {
  description = "Ethereum chain ID for Anvil node"
  type        = number
  default     = 31337
}

variable "anvil_account_count" {
  description = "Number of accounts to create in Anvil node"
  type        = number
  default     = 10
}

variable "anvil_account_balance" {
  description = "Initial balance for each account in Anvil node"
  type        = number
  default     = 100000
}

# Prover Server Configuration
variable "prover_image_repository" {
  description = "Docker image repository for Prover server"
  type        = string
  default     = "elladic/ttc-prover-server"
}

variable "prover_cpu_count" {
  description = "Number of CPUs for Prover server"
  type        = number
  default     = 8  # Maximum allowed in Cloud Run
}

variable "prover_memory_gb" {
  description = "Memory in GB for Prover server"
  type        = number
  default     = 32
}

variable "prover_rust_log_level" {
  description = "Rust log level for Prover server"
  type        = string
  default     = "info"
}

variable "prover_risc0_dev_mode" {
  description = "Enable RISC0 development mode for Prover server"
  type        = string
  default     = "true"
}

# Monitor Server Configuration
variable "monitor_image_repository" {
  description = "Docker image repository for Monitor server"
  type        = string
  default     = "elladic/ttc-monitor-server"
}

variable "monitor_rust_log_level" {
  description = "Rust log level for Monitor server"
  type        = string
  default     = "info"
}

# Cloud SQL Configuration
variable "database_instance_name" {
  description = "Name of the Cloud SQL instance"
  type        = string
  default     = "ttc-postgres"
}

variable "database_version" {
  description = "Database version for Cloud SQL"
  type        = string
  default     = "POSTGRES_15"
}

variable "database_tier" {
  description = "The machine type to use for the database instance"
  type        = string
  default     = "db-f1-micro"
}

variable "database_name" {
  description = "Name of the default database to create"
  type        = string
  default     = "ttc"
}

variable "database_username" {
  description = "Username for the database instance"
  type        = string
  default     = "ttc_user"
}

variable "database_password" {
  description = "Password for the database user"
  type        = string
  sensitive   = true
}

variable "database_deletion_protection" {
  description = "Enable deletion protection for the database instance"
  type        = bool
  default     = false
}

# IAM Configuration
variable "terraform_user_email" {
  description = "Email of the user running terraform (for service account impersonation)"
  type        = string
}

# IAP Configuration
variable "iap_member_list" {
  description = "List of members to grant IAP access (e.g., user:user@example.com)"
  type        = list(string)
  default     = []
}
