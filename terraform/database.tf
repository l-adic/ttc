# First create the private IP address for VPC peering
resource "google_compute_global_address" "private_ip_address" {
  name          = "private-ip-address"
  purpose       = "VPC_PEERING"
  address_type  = "INTERNAL"
  prefix_length = 16
  network       = google_compute_network.vpc.id
  project       = var.gcp_project_id
}

# Then create the VPC peering connection
resource "google_service_networking_connection" "private_vpc_connection" {
  network                 = google_compute_network.vpc.id
  service                 = "servicenetworking.googleapis.com"
  reserved_peering_ranges = [google_compute_global_address.private_ip_address.name]
}

# Finally create the Cloud SQL instance
resource "google_sql_database_instance" "ttc" {
  name             = var.database_instance_name
  database_version = var.database_version
  region           = var.gcp_region
  project          = var.gcp_project_id

  depends_on = [google_service_networking_connection.private_vpc_connection]

  settings {
    tier = var.database_tier

    ip_configuration {
      ipv4_enabled    = true  # Temporarily enable public IP
      authorized_networks {
        name  = "temp-cleanup-access"
        value = "0.0.0.0/0"   # Warning: This allows access from anywhere
      }
      private_network = google_compute_network.vpc.id
    }

    backup_configuration {
      enabled = true
    }
  }

  deletion_protection = var.database_deletion_protection
}

resource "google_sql_database" "ttc" {
  name     = var.database_name
  instance = google_sql_database_instance.ttc.name
  project  = var.gcp_project_id
}

# Create postgres superuser
resource "google_sql_user" "postgres" {
  name     = "postgres"
  instance = google_sql_database_instance.ttc.name
  password = var.database_password
  project  = var.gcp_project_id
  type     = "BUILT_IN"  # This makes it a superuser
}

# Create application user
resource "google_sql_user" "ttc" {
  name     = var.database_username
  instance = google_sql_database_instance.ttc.name
  password = var.database_password
  project  = var.gcp_project_id

  deletion_policy = "ABANDON"  # Don't try to delete the user, just remove from state
}

# Add outputs for database connection information
output "database_instance" {
  description = "The generated instance name"
  value       = google_sql_database_instance.ttc.name
}

output "database_connection_name" {
  description = "The connection name of the instance to be used in connection strings"
  value       = google_sql_database_instance.ttc.connection_name
}

output "database_private_ip" {
  description = "The private IP address of the database instance"
  value       = google_sql_database_instance.ttc.private_ip_address
}

output "database_username" {
  description = "The database username"
  value       = var.database_username
}
