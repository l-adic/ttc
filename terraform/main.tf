terraform {
  required_version = ">= 1.0.0"
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 4.0"
    }
  }
}

# Google Cloud provider configuration
provider "google" {
  # These values should be provided via environment variables or terraform.tfvars
  project = var.gcp_project_id
  region  = var.gcp_region
  zone    = var.gcp_zone
}

# Enable required APIs
resource "google_project_service" "required_apis" {
  for_each = toset([
    "compute.googleapis.com",              # Compute Engine API
    "run.googleapis.com",                  # Cloud Run API
    "iap.googleapis.com",                  # Identity-Aware Proxy API
    "servicenetworking.googleapis.com",    # Service Networking API
    "containerregistry.googleapis.com",    # Container Registry API
    "iam.googleapis.com",                  # Identity and Access Management API
    "cloudresourcemanager.googleapis.com", # Cloud Resource Manager API
    "vpcaccess.googleapis.com"            # Serverless VPC Access API
  ])

  service = each.key
  disable_dependent_services = true
  disable_on_destroy = false
}
