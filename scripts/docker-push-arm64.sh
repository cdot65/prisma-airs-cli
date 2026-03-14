#!/usr/bin/env bash
# Build and push the arm64 Docker image from a local Apple Silicon machine.
# CI handles amd64; run this locally after a release tag to complete multi-arch.
#
# Usage: ./scripts/docker-push-arm64.sh [version]
#   version: semver tag (e.g. 1.4.0). Defaults to package.json version.
#
# Prerequisites:
#   - Docker Desktop running
#   - Logged in to GHCR: echo $GITHUB_TOKEN | docker login ghcr.io -u USERNAME --password-stdin

set -euo pipefail

REGISTRY="ghcr.io"
IMAGE="cdot65/prisma-airs-cli"

# Get version from arg or package.json
VERSION="${1:-$(node -p "require('./package.json').version")}"
MAJOR_MINOR="${VERSION%.*}"

echo "Building arm64 image for ${REGISTRY}/${IMAGE}:${VERSION}"

docker buildx build \
  --platform linux/arm64 \
  --push \
  --tag "${REGISTRY}/${IMAGE}:${VERSION}-arm64" \
  --tag "${REGISTRY}/${IMAGE}:${MAJOR_MINOR}-arm64" \
  .

echo ""
echo "Pushed:"
echo "  ${REGISTRY}/${IMAGE}:${VERSION}-arm64"
echo "  ${REGISTRY}/${IMAGE}:${MAJOR_MINOR}-arm64"
echo ""
echo "To create a multi-arch manifest (after CI pushes amd64):"
echo "  docker buildx imagetools create \\"
echo "    --tag ${REGISTRY}/${IMAGE}:${VERSION} \\"
echo "    ${REGISTRY}/${IMAGE}:${VERSION} \\"
echo "    ${REGISTRY}/${IMAGE}:${VERSION}-arm64"
