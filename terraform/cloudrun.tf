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

    # Request timeout for the service
    timeout = "${var.monitor_prover_timeout_secs}s"

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

      # Increase startup timeout
      startup_probe {
        initial_delay_seconds = 120
        failure_threshold = 20
        period_seconds = 10
        timeout_seconds = 120
        tcp_socket {
          port = 8080
        }
      }
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

# GPU-enabled Cloud Run service - with no health checks at all
resource "google_cloud_run_v2_service" "prover_server_gpu" {
  count    = var.enable_gpu_prover ? 1 : 0
  name     = "prover-server-gpu"
  location = var.gcp_region

  launch_stage = "BETA"

  template {
    # Long timeout
    timeout = "600s"

    containers {
      image = "${var.prover_cuda_image_repository}:${var.docker_cuda_image_tag}"

      resources {
        limits = {
          cpu    = "8"
          memory = "${var.prover_memory_gb}Gi"
          "nvidia.com/gpu" = var.gpu_count
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

      env {
        name  = "NVIDIA_DRIVER_CAPABILITIES"
        value = "compute,utility"
      }

      env {
        name  = "NVIDIA_VISIBLE_DEVICES"
        value = "all"
      }

      env {
        name  = "LD_LIBRARY_PATH"
        value = "/usr/local/cuda/lib64:/usr/local/nvidia/lib64"
      }
      
      # PORT is automatically set by Cloud Run, we don't need to define it
      # Removed explicit PORT setting as it's a reserved env var

      ports {
        container_port = 8080
      }

      env {
        name  = "JSON_RPC_PORT"
        value = "8080"
      }

      # Modified command to ensure it stays running - using proper heredoc syntax
      command = ["/bin/bash"]
      args = ["-c", <<EOT
echo 'Starting prover server...'
touch /tmp/healthy
/app/target/release/prover-server || (echo 'Prover server exited, keeping container alive for debugging' && tail -f /dev/null)
EOT
      ]
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

    # Minimal annotations
    annotations = {
      "run.googleapis.com/execution-environment" = "gen2"
      "run.googleapis.com/startup-cpu-boost" = "true"
      "run.googleapis.com/gpu-driver-version" = "latest"
    }
  }

  # Explicit ingress settings
  ingress = "INGRESS_TRAFFIC_ALL"
  
  # Add dependency on database
  depends_on = [google_sql_database.ttc]
}


# Allow public access to the GPU-enabled Cloud Run service
resource "google_cloud_run_service_iam_member" "noauth_gpu" {
  count    = var.enable_gpu_prover ? 1 : 0
  location = google_cloud_run_v2_service.prover_server_gpu[0].location
  service  = google_cloud_run_v2_service.prover_server_gpu[0].name
  role     = "roles/run.invoker"
  member   = "allUsers"
}

# Allow internal VPC access for GPU service
resource "google_cloud_run_service_iam_member" "vpc_access_gpu" {
  count    = var.enable_gpu_prover ? 1 : 0
  location = google_cloud_run_v2_service.prover_server_gpu[0].location
  service  = google_cloud_run_v2_service.prover_server_gpu[0].name
  role     = "roles/run.invoker"
  member   = "serviceAccount:${google_service_account.prover_server.email}"
}

# IAP configuration for GPU service
resource "google_cloud_run_service_iam_member" "prover_server_gpu_invoker" {
  for_each = var.enable_gpu_prover ? toset(var.iap_member_list) : toset([])

  location = google_cloud_run_v2_service.prover_server_gpu[0].location
  service  = google_cloud_run_v2_service.prover_server_gpu[0].name
  role     = "roles/run.invoker"
  member   = each.value
}
