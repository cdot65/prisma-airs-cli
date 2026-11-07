# airs-mcp production image (M8.1). NOT built or pushed by the agent: the user builds
# and pushes it (M8.3 runbook step 2) and pins the digest in kustomization.yaml.
#   DOCKER_BUILDKIT=1 docker build --platform linux/amd64 -f deploy/airs-mcp/Dockerfile -t <registry>/airs-mcp:<version> .
FROM rust:1.89-bookworm AS build
WORKDIR /repo
COPY . .
RUN cargo build --locked --release -p airs-mcp \
 && sha256sum target/release/airs-mcp > /airs-mcp.sha256

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /repo/target/release/airs-mcp /usr/local/bin/airs-mcp
COPY --from=build /airs-mcp.sha256 /usr/local/share/airs-mcp.sha256
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/airs-mcp"]
