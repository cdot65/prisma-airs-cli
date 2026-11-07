# M0.6 disposable spike image (linux/amd64; cluster architecture per source: amd64).
# Build with the repository root as context (the prototype workspace's cli-link
# probe path-depends on the root crate, so Cargo needs it to resolve --locked);
# Dockerfile.dockerignore keeps target/, .reference/ and private data out:
#   DOCKER_BUILDKIT=1 docker build --platform linux/amd64 \
#     -f prototypes/runtime-mcp/deploy/m0.6/Dockerfile -t registry.cdot.io/airs-mcp/spike:m0.6 .
# Record the pushed digest in the M0.6 receipt and pin it in kustomization.yaml.
FROM rust:1.89-bookworm AS build
WORKDIR /repo
COPY . .
WORKDIR /repo/prototypes/runtime-mcp
RUN cargo build --locked --release -p airs-mcp-spike \
 && sha256sum target/release/airs-mcp-spike > /airs-mcp-spike.sha256

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /repo/prototypes/runtime-mcp/target/release/airs-mcp-spike /usr/local/bin/airs-mcp-spike
COPY --from=build /airs-mcp-spike.sha256 /usr/local/share/airs-mcp-spike.sha256
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/airs-mcp-spike"]
