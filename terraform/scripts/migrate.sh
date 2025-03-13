#!/bin/bash
set -e

# Get database connection info from terraform output
echo "Getting database connection info..."
DB_HOST="34.70.120.157"
DB_NAME="ttc"
DB_USER="ttc_user"
DB_PORT=5432

# Check if password is provided as argument
if [ -z "$1" ]; then
    echo "Usage: $0 <database_password>"
    exit 1
fi
DB_PASSWORD=$1

# Build the migration binaries
echo "Building migration tools..."
cargo build --release --bin create-db
cargo build --release --bin create-schema

# Set environment variables for the migration
export DB_HOST=$DB_HOST
export DB_PORT=$DB_PORT
export DB_USER=$DB_USER
export DB_PASSWORD=$DB_PASSWORD
export DB_NAME="postgres"  # Initial connection to postgres database
export DB_CREATE_NAME="ttc"  # The database we want to create

echo "Running database creation..."
./target/release/create-db

# Switch to the ttc database for schema creation
export DB_NAME="ttc"

echo "Running schema migration..."
./target/release/create-schema

echo "Migration completed successfully!"
