# Service account for the Monitor server
resource "google_service_account" "monitor_server" {
  account_id   = "monitor-server-sa"
  display_name = "Monitor Server Service Account"
}

# Instance template for the Monitor server
resource "google_compute_instance_template" "monitor_server" {
  name_prefix  = "monitor-server-template-"
  machine_type = "e2-standard-2"  # 2 vCPU, 8GB RAM
  depends_on = [
    google_compute_forwarding_rule.ethereum_node,
    google_compute_instance.prover_server_gpu[0]
  ]
  
  disk {
    source_image = "cos-cloud/cos-stable"
    auto_delete  = true
    boot         = true
    disk_size_gb = 50
  }

  network_interface {
    network    = google_compute_network.vpc.id
    subnetwork = google_compute_subnetwork.subnet.id
  }

  metadata = {
    user-data = <<EOF
#cloud-config
write_files:
- path: /etc/systemd/system/monitor.service
  permissions: 0644
  owner: root
  content: |
    [Unit]
    Description=TTC Monitor Server
    After=docker.service
    Requires=docker.service

    [Service]
    # Increase timeouts
    TimeoutStartSec=600
    TimeoutStopSec=6`:windows00
    
    Environment="RUST_LOG=${var.monitor_rust_log_level}"
    Environment="DB_HOST=${google_sql_database_instance.ttc.private_ip_address}"
    Environment="DB_PORT=5432"
    Environment="DB_USER=${var.database_username}"
    Environment="DB_PASSWORD=${var.database_password}"
    Environment="DB_NAME=${var.database_name}"
    Environment="NODE_HOST=${google_compute_forwarding_rule.ethereum_node.ip_address}"
    Environment="NODE_PORT=8545"
    Environment="PROVER_PROTOCOL=http"
    Environment="PROVER_HOST=${google_compute_instance.prover_server_gpu[0].network_interface[0].network_ip}"
    Environment="PROVER_TIMEOUT_SECS=${var.monitor_prover_timeout_secs}"
    ExecStartPre=/usr/bin/timeout 300 /usr/bin/docker pull ${var.monitor_image_repository}:${var.docker_image_tag}
    ExecStart=/bin/bash -c '\
      /usr/bin/docker run --rm --name monitor \
      -p 3030:3030 \
      -e JSON_RPC_PORT=3030 \
      -e RUST_LOG=$${RUST_LOG} \
      -e DB_HOST=$${DB_HOST} \
      -e DB_PORT=$${DB_PORT} \
      -e DB_USER=$${DB_USER} \
      -e DB_PASSWORD=$${DB_PASSWORD} \
      -e DB_NAME=$${DB_NAME} \
      -e NODE_HOST=$${NODE_HOST} \
      -e NODE_PORT=$${NODE_PORT} \
      -e PROVER_PROTOCOL=$${PROVER_PROTOCOL} \
      -e PROVER_HOST=$${PROVER_HOST} \
      -e PROVER_TIMEOUT_SECS=$${PROVER_TIMEOUT_SECS} \
      ${var.monitor_image_repository}:${var.docker_image_tag} \
      /app/target/release/monitor-server'
    ExecStop=/usr/bin/docker stop monitor
    Restart=always
    RestartSec=5

    [Install]
    WantedBy=multi-user.target

runcmd:
  - systemctl daemon-reload
  - systemctl enable monitor.service
  - systemctl start monitor.service
EOF
  }

  service_account {
    email  = google_service_account.monitor_server.email
    scopes = ["cloud-platform"]
  }

  tags = ["ssh", "monitor-server", "iap-tunnel"]
}

# Managed instance group for auto-restart
resource "google_compute_instance_group_manager" "monitor_server" {
  name = "monitor-server-mig"
  zone = var.gcp_zone
  
  base_instance_name = "monitor-server"
  target_size       = 1

  version {
    instance_template = google_compute_instance_template.monitor_server.id
  }

  named_port {
    name = "monitor"
    port = 3030
  }

  depends_on = [google_sql_database.ttc]
}

# Health check for the instance group
resource "google_compute_health_check" "monitor_server" {
  name                = "monitor-server-health-check"
  check_interval_sec  = 5
  timeout_sec         = 5
  healthy_threshold   = 2
  unhealthy_threshold = 10

  tcp_health_check {
    port = 3030
  }
}

# Internal load balancer for the Monitor server
resource "google_compute_region_backend_service" "monitor_server" {
  name                  = "monitor-server-backend"
  region                = var.gcp_region
  protocol              = "TCP"
  load_balancing_scheme = "INTERNAL"
  health_checks         = [google_compute_health_check.monitor_server.id]

  backend {
    group = google_compute_instance_group_manager.monitor_server.instance_group
  }
}

resource "google_compute_forwarding_rule" "monitor_server" {
  name                  = "monitor-server-forwarding-rule"
  region                = var.gcp_region
  network               = google_compute_network.vpc.id
  subnetwork            = google_compute_subnetwork.subnet.id
  backend_service       = google_compute_region_backend_service.monitor_server.id
  ports                 = ["3030"]
  load_balancing_scheme = "INTERNAL"
}
