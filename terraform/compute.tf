# Service account for the Ethereum node
resource "google_service_account" "ethereum_node" {
  account_id   = "ethereum-node-sa"
  display_name = "Ethereum Node Service Account"
}

# Instance template for the Ethereum node
resource "google_compute_instance_template" "ethereum_node" {
  name_prefix  = "ethereum-node-template-"
  machine_type = var.anvil_machine_type
  
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
  network               = google_compute_network.vpc.id
  subnetwork            = google_compute_subnetwork.subnet.id
  backend_service       = google_compute_region_backend_service.ethereum_node.id
  ports                 = ["8545"]
  load_balancing_scheme = "INTERNAL"
}
