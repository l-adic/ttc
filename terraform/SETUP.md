# Google Cloud Setup Guide

This guide walks through the necessary steps to set up Google Cloud for deploying the TTC project on Ubuntu.

## 1. Create a Google Cloud Project

1. Go to the [Google Cloud Console](https://console.cloud.google.com)
2. Click the project dropdown at the top and select "New Project"
3. Enter a project name and note the project ID
4. Wait for the project to be created

## 2. Enable Billing

1. Go to [Billing](https://console.cloud.google.com/billing)
2. Link your project to a billing account
3. If you don't have a billing account, create one and add a payment method

## 3. Install Required Tools

### Install Google Cloud SDK
```bash
# Add the Cloud SDK distribution URI as a package source
echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] https://packages.cloud.google.com/apt cloud-sdk main" | sudo tee -a /etc/apt/sources.list.d/google-cloud-sdk.list

# Import the Google Cloud public key
curl https://packages.cloud.google.com/apt/doc/apt-key.gpg | sudo apt-key --keyring /usr/share/keyrings/cloud.google.gpg add -

# Update and install the Cloud SDK
sudo apt-get update && sudo apt-get install google-cloud-cli
```

### Install Terraform
```bash
# Add HashiCorp GPG key
wget -O- https://apt.releases.hashicorp.com/gpg | sudo gpg --dearmor -o /usr/share/keyrings/hashicorp-archive-keyring.gpg

# Add HashiCorp repository
echo "deb [signed-by=/usr/share/keyrings/hashicorp-archive-keyring.gpg] https://apt.releases.hashicorp.com $(lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/hashicorp.list

# Update and install Terraform
sudo apt update && sudo apt install terraform
```

## 4. Configure Google Cloud SDK

```bash
# Log in to Google Cloud
gcloud auth login

# Set your project
gcloud config set project YOUR_PROJECT_ID

# Create application default credentials
gcloud auth application-default login
```

## 5. Enable Required APIs

You can enable these manually in the console or let Terraform handle it (as configured in main.tf):

```bash
# Enable required APIs
gcloud services enable compute.googleapis.com \
    cloudrun.googleapis.com \
    iap.googleapis.com \
    servicenetworking.googleapis.com \
    containerregistry.googleapis.com
```

## 6. Set Up IAP Access

1. Go to [OAuth Consent Screen](https://console.cloud.google.com/apis/credentials/consent)
2. Choose "Internal" if you have Google Workspace, or "External" if not
3. Fill in the required information:
   - App name: "TTC Infrastructure"
   - User support email: Your email
   - Developer contact email: Your email
4. No additional scopes needed
5. Add your email to test users if using External

## 7. Create Service Account for Terraform

```bash
# Create service account
gcloud iam service-accounts create terraform-deployer \
    --display-name "Terraform Deployer"

# Grant necessary roles
gcloud projects add-iam-policy-binding YOUR_PROJECT_ID \
    --member="serviceAccount:terraform-deployer@YOUR_PROJECT_ID.iam.gserviceaccount.com" \
    --role="roles/editor"

gcloud projects add-iam-policy-binding YOUR_PROJECT_ID \
    --member="serviceAccount:terraform-deployer@YOUR_PROJECT_ID.iam.gserviceaccount.com" \
    --role="roles/compute.networkAdmin"

gcloud projects add-iam-policy-binding YOUR_PROJECT_ID \
    --member="serviceAccount:terraform-deployer@YOUR_PROJECT_ID.iam.gserviceaccount.com" \
    --role="roles/run.admin"

# Create and download key
gcloud iam service-accounts keys create terraform-key.json \
    --iam-account=terraform-deployer@YOUR_PROJECT_ID.iam.gserviceaccount.com
```

## 8. Configure Terraform Authentication

1. Move the key file to a secure location:
```bash
mkdir -p ~/.config/gcloud
mv terraform-key.json ~/.config/gcloud/
chmod 600 ~/.config/gcloud/terraform-key.json
```

2. Set the environment variable:
```bash
export GOOGLE_APPLICATION_CREDENTIALS="$HOME/.config/gcloud/terraform-key.json"
```

Add this to your shell's rc file:
```bash
echo 'export GOOGLE_APPLICATION_CREDENTIALS="$HOME/.config/gcloud/terraform-key.json"' >> ~/.bashrc
# Or if using zsh:
# echo 'export GOOGLE_APPLICATION_CREDENTIALS="$HOME/.config/gcloud/terraform-key.json"' >> ~/.zshrc
```

## 9. Create terraform.tfvars

Copy the example file and update with your values:
```bash
cp terraform.tfvars.example terraform.tfvars
```

Edit terraform.tfvars with:
- Your GCP project ID
- Your preferred region/zone
- Your email for IAP access
- Any other customizations needed

## 10. Ready to Deploy

You can now proceed with:
```bash
# Initialize Terraform
terraform init

# Review the plan
terraform plan

# Apply the configuration
terraform apply
```

## Troubleshooting

1. If you get permission errors:
   - Verify the service account has the correct roles
   - Check that GOOGLE_APPLICATION_CREDENTIALS is set correctly
   - Ensure the key file is readable

2. If APIs are not enabled:
   - Wait a few minutes after enabling them
   - Check the project ID is correct
   - Verify billing is enabled

3. If IAP doesn't work:
   - Verify OAuth consent screen is configured
   - Check that your email is in the IAP members list
   - Ensure you're logged in with `gcloud auth login`
