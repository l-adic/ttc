# Service account for Prover Server
resource "google_service_account" "prover_server" {
  account_id   = "prover-server-sa"
  display_name = "Prover Server Service Account"
}

# Cloud Run service
resource "google_cloud_run_v2_service" "prover_server" {
  name     = "prover-server"
  location = var.gcp_region
  
  template {
    containers {
      image = "${var.prover_image_repository}:${var.docker_image_tag}"
      
      resources {
        limits = {
          cpu    = "8"  # Maximum allowed CPU
          memory = "${var.prover_memory_gb}Gi"
        }
      }

      env {
        name  = "RUST_LOG"
        value = var.prover_rust_log_level
      }
      
      env {
        name  = "RISC0_DEV_MODE"
        value = var.prover_risc0_dev_mode
      }

      env {
        name  = "DB_HOST"
        value = google_sql_database_instance.ttc.private_ip_address
      }

      env {
        name  = "DB_PORT"
        value = "5432"
      }

      env {
        name  = "DB_USER"
        value = var.database_username
      }

      env {
        name  = "DB_PASSWORD"
        value = var.database_password
      }

      env {
        name  = "DB_NAME"
        value = var.database_name
      }

      env {
        name  = "NODE_HOST"
        value = google_compute_forwarding_rule.ethereum_node.ip_address
      }

      env {
        name  = "NODE_PORT"
        value = "8545"
      }

      # Let Cloud Run use its default port 8080
      ports {
        container_port = 8080
      }

      # Set JSON_RPC_PORT to match Cloud Run's default port
      env {
        name  = "JSON_RPC_PORT"
        value = "8080"
      }

      # Command and arguments for the container
      command = ["/app/target/release/prover-server"]
    }

    scaling {
      min_instance_count = 0
      max_instance_count = 1
    }

    vpc_access {
      connector = google_vpc_access_connector.connector.id
      egress    = "PRIVATE_RANGES_ONLY"
    }

    service_account = google_service_account.prover_server.email
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }

  depends_on = [google_sql_database.ttc]

  # Configure ingress settings at the service level
  ingress = "INGRESS_TRAFFIC_ALL"
}

# VPC Access Connector
resource "google_vpc_access_connector" "connector" {
  name          = "${var.vpc_network_name}-connector"
  region        = var.gcp_region
  ip_cidr_range = "10.8.0.0/28"
  network       = google_compute_network.vpc.id
}

# Allow public access to the Cloud Run service
resource "google_cloud_run_service_iam_member" "noauth" {
  location = google_cloud_run_v2_service.prover_server.location
  service  = google_cloud_run_v2_service.prover_server.name
  role     = "roles/run.invoker"
  member   = "allUsers"
}
