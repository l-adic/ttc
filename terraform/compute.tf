# Service account for GPU Prover
resource "google_service_account" "prover_server" {
  account_id   = "prover-server-sa"
  display_name = "Prover Server Service Account"
}

# Firewall rule for SSH access
resource "google_compute_firewall" "allow_ssh_gpu" {
  count   = var.enable_gpu_prover ? 1 : 0
  name    = "allow-ssh-gpu"
  network = google_compute_network.vpc.id

  allow {
    protocol = "tcp"
    ports    = ["22"]
  }

  source_ranges = ["0.0.0.0/0"]  # You might want to restrict this to your IP
  target_tags   = ["gpu-prover"]
}

# Firewall rule for Prover JSON-RPC
resource "google_compute_firewall" "allow_prover_rpc" {
  count   = var.enable_gpu_prover ? 1 : 0
  name    = "allow-prover-rpc"
  network = google_compute_network.vpc.id

  allow {
    protocol = "tcp"
    ports    = ["3000"]
  }

  source_ranges = ["0.0.0.0/0"]  # You might want to restrict this to your IP
  target_tags   = ["gpu-prover"]
}

# GPU-enabled Prover Server using NVIDIA NGC VM Image
resource "google_compute_instance" "prover_server_gpu" {
  count        = var.enable_gpu_prover ? 1 : 0
  name         = "prover-server-gpu"
  machine_type = "g2-standard-8"    # G2 series with higher base clock speed (4.0 GHz)
  zone         = var.gcp_zone
  tags         = ["gpu-prover"]     # For firewall rules

  boot_disk {
    initialize_params {
      image = "projects/nvidia-ngc-public/global/images/nvidia-gpu-cloud-vmi-base-2024-10-1-x86-64"
      size  = 128  # GB
    }
  }

  guest_accelerator {
    type  = "nvidia-l4"    # L4 GPU - newer generation, better performance than T4
    count = var.gpu_count
  }

  scheduling {
    on_host_maintenance = "TERMINATE"
    automatic_restart   = true
  }

  network_interface {
    network    = google_compute_network.vpc.id
    subnetwork = google_compute_subnetwork.subnet.id

    # Add external IP for SSH access
    access_config {
      // Ephemeral public IP
    }
  }

  metadata = {
    ssh-keys = "${var.ssh_user}:${file(var.ssh_pub_key_path)}"

    startup-script = "yes y | /opt/nvidia/gcp-ngc-login.sh true none false /var/log/nvidia 2> /dev/null"

    user-data = <<EOF
#cloud-config
write_files:
- path: /etc/systemd/system/prover.service
  permissions: 0644
  owner: root
  content: |
    [Unit]
    Description=TTC Prover Server
    After=docker.service
    Requires=docker.service

    [Service]
    # Increase timeouts
    TimeoutStartSec=600
    TimeoutStopSec=6`:windows00
    
    Environment="RUST_LOG=${var.prover_rust_log_level}"
    Environment="RISC0_DEV_MODE=${var.prover_risc0_dev_mode}"
    Environment="DB_HOST=${google_sql_database_instance.ttc.private_ip_address}"
    Environment="DB_USER=${var.database_username}"
    Environment="DB_PASSWORD=${var.database_password}"
    Environment="DB_NAME=${var.database_name}"
    Environment="NODE_HOST=${google_compute_forwarding_rule.ethereum_node.ip_address}"
    ExecStartPre=/usr/bin/docker pull ${var.prover_cuda_image_repository}:${var.docker_cuda_image_tag}
    ExecStart=/bin/bash -c '\
      /usr/bin/docker run --rm --rm --name prover-server \
        --gpus all \
        -p 3000:3000 \
        -e RUST_LOG=$${RUST_LOG} \
        -e RISC0_DEV_MODE=$${RISC0_DEV_MODE} \
        -e DB_HOST=$${DB_HOST} \
        -e DB_PORT=5432 \
        -e DB_USER=$${DB_USER} \
        -e DB_PASSWORD=$${DB_PASSWORD} \
        -e DB_NAME=$${DB_NAME} \
        -e NODE_HOST=$${NODE_HOST} \
        -e NODE_PORT=8545 \
        -e JSON_RPC_PORT=3000 \
        -e NVIDIA_VISIBLE_DEVICES=all \
        -e NVIDIA_DRIVER_CAPABILITIES=all \
        -e RISC0_PROVER=local \
        -e RUST_BACKTRACE=1 \
        -e RISC0_WORK_DIR=/tmp/risc0-work-dir \
        -v /var/run/docker.sock:/var/run/docker.sock \
        -v /tmp/risc0-work-dir:/tmp/risc0-work-dir \
        --privileged \
        ${var.prover_cuda_image_repository}:${var.docker_cuda_image_tag}'
    ExecStop=/usr/bin/docker stop prover-server
    Restart=always
    RestartSec=5

    [Install]
    WantedBy=multi-user.target

runcmd:
  - systemctl daemon-reload
  - systemctl enable prover.service
  - systemctl start prover.service
EOF
  }

  service_account {
    email  = google_service_account.prover_server.email
    scopes = ["cloud-platform"]
  }

  allow_stopping_for_update = true

  depends_on = [
    google_compute_forwarding_rule.ethereum_node,
    google_sql_database.ttc
  ]
}

# Service account for the Ethereum node
resource "google_service_account" "ethereum_node" {
  account_id   = "ethereum-node-sa"
  display_name = "Ethereum Node Service Account"
}

# Instance template for the Ethereum node
resource "google_compute_instance_template" "ethereum_node" {
  name_prefix  = "ethereum-node-template-"
  machine_type = var.anvil_machine_type
  depends_on   = [google_service_account.ethereum_node]
  
  # Use preemptible VM
  scheduling {
    preemptible         = true
    automatic_restart   = false
    on_host_maintenance = "TERMINATE"
  }

  disk {
    source_image = "cos-cloud/cos-stable"
    auto_delete  = true
    boot         = true
    disk_size_gb = 10
  }

  network_interface {
    network    = google_compute_network.vpc.id
    subnetwork = google_compute_subnetwork.subnet.id
  }

  metadata = {
    user-data = <<EOF
#cloud-config
write_files:
- path: /etc/systemd/system/anvil.service
  permissions: 0644
  owner: root
  content: |
    [Unit]
    Description=Anvil Ethereum Node
    After=docker.service
    Requires=docker.service

    [Service]
    Environment="CHAIN_ID=${var.anvil_chain_id}"
    Environment="ACCOUNTS=${var.anvil_account_count}"
    Environment="BALANCE=${var.anvil_account_balance}"
    ExecStartPre=/usr/bin/docker pull ${var.anvil_image_repository}:${var.docker_image_tag}
    ExecStart=/usr/bin/docker run --rm --name anvil \
      -p 8545:8545 \
      -e CHAIN_ID=$${CHAIN_ID} \
      -e ACCOUNTS=$${ACCOUNTS} \
      -e BALANCE=$${BALANCE} \
      ${var.anvil_image_repository}:${var.docker_image_tag} \
      anvil \
      --host 0.0.0.0 \
      --port 8545 \
      --chain-id $${CHAIN_ID} \
      --accounts $${ACCOUNTS} \
      --balance $${BALANCE}
    ExecStop=/usr/bin/docker stop anvil
    Restart=always
    RestartSec=5

    [Install]
    WantedBy=multi-user.target

runcmd:
  - systemctl daemon-reload
  - systemctl enable anvil.service
  - systemctl start anvil.service
EOF
  }

  service_account {
    email  = google_service_account.ethereum_node.email
    scopes = ["cloud-platform"]
  }

  tags = ["ssh", "ethereum-node"]

  lifecycle {
    create_before_destroy = true
  }
}

# Managed instance group for auto-restart
resource "google_compute_instance_group_manager" "ethereum_node" {
  name = "ethereum-node-mig"
  zone = var.gcp_zone
  depends_on = [google_service_account.ethereum_node]
  
  base_instance_name = "ethereum-node"
  target_size       = 1

  version {
    instance_template = google_compute_instance_template.ethereum_node.id
  }

  named_port {
    name = "ethereum"
    port = 8545
  }
}

# Health check for the instance group
resource "google_compute_health_check" "ethereum_node" {
  name                = "ethereum-node-health-check"
  check_interval_sec  = 5
  timeout_sec         = 5
  healthy_threshold   = 2
  unhealthy_threshold = 10

  tcp_health_check {
    port = 8545
  }
}

# Internal load balancer for the Ethereum node
resource "google_compute_region_backend_service" "ethereum_node" {
  name                  = "ethereum-node-backend"
  region                = var.gcp_region
  depends_on = [google_service_account.ethereum_node]
  protocol              = "TCP"
  load_balancing_scheme = "INTERNAL"
  health_checks         = [google_compute_health_check.ethereum_node.id]

  backend {
    group = google_compute_instance_group_manager.ethereum_node.instance_group
  }
}

resource "google_compute_forwarding_rule" "ethereum_node" {
  name                  = "ethereum-node-forwarding-rule"
  region                = var.gcp_region
  depends_on = [google_service_account.ethereum_node]
  network               = google_compute_network.vpc.id
  subnetwork            = google_compute_subnetwork.subnet.id
  backend_service       = google_compute_region_backend_service.ethereum_node.id
  ports                 = ["8545"]
  load_balancing_scheme = "INTERNAL"
}
