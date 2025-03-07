# Service account for Cloud Run
resource "google_service_account" "prover_server" {
  account_id   = "prover-server-sa"
  display_name = "Prover Server Service Account"
}

# CPU-only Cloud Run service
resource "google_cloud_run_v2_service" "prover_server_cpu" {
  name     = "prover-server-cpu"
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

      ports {
        container_port = 8546
      }

      # Command and arguments for the container
      command = ["/app/target/release/prover-server"]
      args = [
        "--node-url", "http://${google_compute_forwarding_rule.ethereum_node.ip_address}:8545",
        "--json-rpc-port", "8546"
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
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }
}

# VPC Access Connector
resource "google_vpc_access_connector" "connector" {
  name          = "${var.vpc_network_name}-connector"
  region        = var.gcp_region
  ip_cidr_range = "10.8.0.0/28"
  network       = google_compute_network.vpc.id
}

# Allow internal VPC access for CPU service
resource "google_cloud_run_service_iam_member" "vpc_access_cpu" {
  location = google_cloud_run_v2_service.prover_server_cpu.location
  service  = google_cloud_run_v2_service.prover_server_cpu.name
  role     = "roles/run.invoker"
  member   = "serviceAccount:${google_service_account.prover_server.email}"
}

# IAP configuration for CPU service
resource "google_cloud_run_service_iam_member" "prover_server_cpu_invoker" {
  for_each = toset(var.iap_member_list)
  
  location = google_cloud_run_v2_service.prover_server_cpu.location
  service  = google_cloud_run_v2_service.prover_server_cpu.name
  role     = "roles/run.invoker"
  member   = each.value
}

# GPU-enabled Cloud Run service
resource "google_cloud_run_v2_service" "prover_server_gpu" {
  count    = var.enable_gpu_prover ? 1 : 0
  name     = "prover-server-gpu"
  location = var.gcp_region
  
  template {
    containers {
      image = "${var.prover_cuda_image_repository}:${var.docker_cuda_image_tag}"
      
      resources {
        limits = {
          cpu    = "8"  # Maximum allowed CPU
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
        name  = "NVIDIA_DRIVER_CAPABILITIES"
        value = "compute,utility"
      }

      env {
        name  = "NVIDIA_VISIBLE_DEVICES"
        value = "all"
      }

      ports {
        container_port = 8546
      }

      # Command and arguments for the container
      command = ["/app/target/release/prover-server"]
      args = [
        "--node-url", "http://${google_compute_forwarding_rule.ethereum_node.ip_address}:8545",
        "--json-rpc-port", "8546"
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

    annotations = {
      "run.googleapis.com/startup-cpu-boost" = "true"
      "run.googleapis.com/execution-environment" = "gen2"
      "run.googleapis.com/vpc-access-connector" = google_vpc_access_connector.connector.id
      "run.googleapis.com/vpc-access-egress" = "private-ranges-only"
      "run.googleapis.com/use-zonal-gpu" = "true"  # Disable zonal redundancy for GPU support
    }
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }
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
