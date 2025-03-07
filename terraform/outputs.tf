# VPC Network outputs
output "vpc_network" {
  description = "The VPC network"
  value       = google_compute_network.vpc.name
}

output "vpc_subnet" {
  description = "The VPC subnet"
  value       = google_compute_subnetwork.subnet.name
}

# Anvil Node outputs
output "anvil_node_address" {
  description = "Internal IP address of the Anvil node load balancer"
  value       = google_compute_forwarding_rule.ethereum_node.ip_address
}

output "anvil_node_port" {
  description = "Port for the Anvil node"
  value       = "8545"
}

output "anvil_instance_name" {
  description = "Name of the Anvil instance"
  value       = "${google_compute_instance_group_manager.ethereum_node.base_instance_name}-${google_compute_instance_group_manager.ethereum_node.instance_group}"
}

# Prover Server outputs
output "prover_server_cpu_url" {
  description = "URL of the CPU-only Cloud Run service"
  value       = google_cloud_run_v2_service.prover_server_cpu.uri
}

output "prover_server_gpu_url" {
  description = "URL of the GPU-enabled Cloud Run service"
  value       = var.enable_gpu_prover ? google_cloud_run_v2_service.prover_server_gpu[0].uri : "GPU prover not enabled"
}

output "prover_server_port" {
  description = "Port for the Prover servers"
  value       = "8546"
}

# Connection Instructions
output "connection_instructions" {
  description = "Instructions for connecting to the services"
  value       = <<EOT
To connect to the services:

1. Ensure you have the Google Cloud SDK installed and are authenticated:
   gcloud auth login
   gcloud config set project ${var.gcp_project_id}

2. Open two terminal windows and run the following commands:

   Terminal 1 (Anvil Node):
   gcloud compute start-iap-tunnel ${google_compute_instance_group_manager.ethereum_node.base_instance_name}-xxxx 8545 --local-host-port=localhost:8545 --zone=${var.gcp_zone}
   (Note: Get the full instance name by running the command in step 4)

   Terminal 2 (CPU Prover Server):
   gcloud run services proxy prover-server-cpu --port=8546 --region=${var.gcp_region}

   Terminal 3 (GPU Prover Server, if enabled):
   gcloud run services proxy prover-server-gpu --port=8547 --region=${var.gcp_region}

3. You can now access:
   - Anvil Node at http://localhost:8545
   - CPU Prover Server at http://localhost:8546
   - GPU Prover Server at http://localhost:8547 (if enabled)

4. To get the actual Anvil instance name, run:
   gcloud compute instances list --filter="name~'${google_compute_instance_group_manager.ethereum_node.base_instance_name}'" --zones=${var.gcp_zone}

Note: Keep the terminal windows open to maintain the connections.
EOT
}
