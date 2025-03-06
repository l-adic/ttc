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

variable "anvil_image_repository" {
  description = "Docker image repository for Anvil node"
  type        = string
  default     = "elladic/anvil-node"
}

variable "prover_image_repository" {
  description = "Docker image repository for Prover server"
  type        = string
  default     = "elladic/ttc-prover-server"
}

# Anvil Node Configuration
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
variable "prover_cpu_count" {
  description = "Number of CPUs for Prover server"
  type        = number
  default     = 16
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

# IAP Configuration
variable "iap_member_list" {
  description = "List of members to grant IAP access (e.g., user:user@example.com)"
  type        = list(string)
  default     = []
}
