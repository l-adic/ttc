# VPC Network
resource "google_compute_network" "vpc" {
  name                    = var.vpc_network_name
  auto_create_subnetworks = false
  depends_on             = [google_project_service.required_apis]
}

# Subnet
resource "google_compute_subnetwork" "subnet" {
  name          = "${var.vpc_network_name}-subnet"
  ip_cidr_range = "10.0.0.0/24"
  network       = google_compute_network.vpc.id
  region        = var.gcp_region

  # Enable private Google access for Cloud Run
  private_ip_google_access = true
}

# Cloud NAT for internet access from private instances
resource "google_compute_router" "router" {
  name    = "${var.vpc_network_name}-router"
  network = google_compute_network.vpc.id
  region  = var.gcp_region
}

resource "google_compute_router_nat" "nat" {
  name                               = "${var.vpc_network_name}-nat"
  router                            = google_compute_router.router.name
  region                            = var.gcp_region
  nat_ip_allocate_option           = "AUTO_ONLY"
  source_subnetwork_ip_ranges_to_nat = "ALL_SUBNETWORKS_ALL_IP_RANGES"
}

# Firewall rules
resource "google_compute_firewall" "iap_ssh" {
  name    = "${var.vpc_network_name}-allow-iap-ssh"
  network = google_compute_network.vpc.id

  allow {
    protocol = "tcp"
    ports    = ["22"]
  }

  source_ranges = ["35.235.240.0/20"] # IAP's IP range
  target_tags   = ["ssh"]
}

# Allow IAP tunneling to Anvil port
resource "google_compute_firewall" "iap_tcp" {
  name    = "${var.vpc_network_name}-allow-iap-tcp"
  network = google_compute_network.vpc.id

  allow {
    protocol = "tcp"
    ports    = ["8545"]  # Anvil port
  }

  source_ranges = ["35.235.240.0/20"]  # IAP's IP range
  target_tags   = ["ethereum-node"]
}

resource "google_compute_firewall" "internal" {
  name    = "${var.vpc_network_name}-allow-internal"
  network = google_compute_network.vpc.id

  allow {
    protocol = "tcp"
    ports    = ["0-65535"]
  }

  allow {
    protocol = "udp"
    ports    = ["0-65535"]
  }

  allow {
    protocol = "icmp"
  }

  source_ranges = ["10.0.0.0/24"]
}

# IAP SSH permissions
resource "google_project_iam_member" "iap_tunnel_user" {
  for_each = toset(var.iap_member_list)
  project  = var.gcp_project_id
  role     = "roles/iap.tunnelResourceAccessor"
  member   = each.value
}
