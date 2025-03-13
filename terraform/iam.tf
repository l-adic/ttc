# Grant Cloud SQL Admin role to the terraform user
resource "google_project_iam_member" "user_cloudsql_admin" {
  project = var.gcp_project_id
  role    = "roles/cloudsql.admin"
  member  = "user:${var.terraform_user_email}"
}

# Grant Network Admin role to the terraform user
resource "google_project_iam_member" "user_network_admin" {
  project = var.gcp_project_id
  role    = "roles/compute.networkAdmin"
  member  = "user:${var.terraform_user_email}"
}

# Grant Service Networking Admin role to the terraform user
resource "google_project_iam_member" "user_service_networking_admin" {
  project = var.gcp_project_id
  role    = "roles/servicenetworking.networksAdmin"
  member  = "user:${var.terraform_user_email}"
}
