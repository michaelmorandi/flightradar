#!/bin/bash

# Get git version information (tag if exists, otherwise commit hash)
# --tags: look for tags
# --always: fall back to commit hash if no tag found
# --dirty: append "-dirty" if working directory has uncommitted changes
COMMIT_ID=$(git describe --tags --always --dirty)

# Build the Docker image with the version information
docker build -t flightradar:latest --build-arg COMMIT_ID=$COMMIT_ID .

echo "Built flightradar:latest with version: $COMMIT_ID"