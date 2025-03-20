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

# GPU Prover Server outputs
output "prover_server_gpu_internal_ip" {
  description = "Internal IP address of the GPU Prover Server"
  value       = var.enable_gpu_prover ? google_compute_instance.prover_server_gpu[0].network_interface[0].network_ip : "GPU prover not enabled"
}

output "prover_server_gpu_external_ip" {
  description = "External IP address for SSH access to GPU Prover Server"
  value       = var.enable_gpu_prover ? google_compute_instance.prover_server_gpu[0].network_interface[0].access_config[0].nat_ip : "GPU prover not enabled"
}

# Connection Instructions
output "connection_instructions" {
  description = "Instructions for connecting to the services"
  value       = <<EOT
To connect to the services:

1. Ensure you have the Google Cloud SDK installed and are authenticated:
   gcloud auth login
   gcloud config set project ${var.gcp_project_id}

2. Open three terminal windows and run the following commands:

   Terminal 1 (Anvil Node):
   gcloud compute start-iap-tunnel ${google_compute_instance_group_manager.ethereum_node.base_instance_name}-xxxx 8545 --local-host-port=localhost:8545 --zone=${var.gcp_zone}
   (Note: Get the full instance name by running the command in step 4)

   Terminal 2 (Monitor Server):
   gcloud compute start-iap-tunnel ${google_compute_instance_group_manager.monitor_server.base_instance_name}-xxxx 3030 --local-host-port=localhost:3030 --zone=${var.gcp_zone}
   (Note: Get the full instance name by running the command in step 4)

   Terminal 3 (GPU Prover Server, if enabled):
   # SSH into the GPU instance
   ssh ${var.ssh_user}@${google_compute_instance.prover_server_gpu[0].network_interface[0].access_config[0].nat_ip}

   # Check prover logs
   check-prover-logs

   # Access the service internally
   curl http://${google_compute_instance.prover_server_gpu[0].network_interface[0].network_ip}:3000/health_check

3. You can now access:
   - Anvil Node at http://localhost:8545
   - Monitor Server at http://localhost:3030
   - GPU Prover Server:
     * Internal access: http://${google_compute_instance.prover_server_gpu[0].network_interface[0].network_ip}:3000
     * SSH access: ssh ${var.ssh_user}@${google_compute_instance.prover_server_gpu[0].network_interface[0].access_config[0].nat_ip}

Note: The GPU prover server is a dedicated instance that can be started/stopped as needed.

4. To get the actual instance names for compute instances, run:
   gcloud compute instances list --filter="name~'${google_compute_instance_group_manager.ethereum_node.base_instance_name}|${google_compute_instance_group_manager.monitor_server.base_instance_name}'" --zones=${var.gcp_zone}

Note: Keep the terminal windows open to maintain the connections.
EOT
}
