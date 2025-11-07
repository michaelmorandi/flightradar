#!/bin/bash

# Generate meta.json before Docker build
# This ensures version info is always captured, regardless of how docker build is called

COMMIT_ID=$(git describe --tags --always --dirty 2>/dev/null || echo "unknown")
BUILD_TIMESTAMP=$(date -Iseconds)

mkdir -p resources

cat > resources/meta.json << EOF
{
  "commit_id": "${COMMIT_ID}",
  "build_timestamp": "${BUILD_TIMESTAMP}"
}
EOF

echo "Generated resources/meta.json with commit_id: ${COMMIT_ID}"
