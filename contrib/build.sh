#!/bin/bash

# Generate meta.json with git version information
./contrib/generate-meta.sh

# Get the version for display purposes
COMMIT_ID=$(git describe --tags --always --dirty 2>/dev/null || echo "unknown")

# Build the Docker image
docker build -t flightradar:latest .

echo "Built flightradar:latest with version: $COMMIT_ID"