# deploy/airs-mcp: GitOps artifacts for the optional runtime MCP service (M8.1)

- **Not applied by agents.** Nothing here has been applied, built or pushed by an agent.
  The owner deploys it by following docs/RUNTIME_MCP_DEPLOYMENT_RUNBOOK.md.
- **Format.** The manifests are JSON, which is valid YAML.
- **Validation.** `cargo test -p airs-mcp --test deploy_manifests` checks them statically
  (no cluster contact):
  - the hardened singleton;
  - storage, secrets, probes, metrics, network and exposure;
  - the configuration template, through the service's own strict parser.

| File | What |
|---|---|
| `namespace.json` | `airs-mcp` namespace, Pod Security `baseline` (the CHOWN-only init container) |
| `pvc.json` | 100 Gi `ReadWriteOncePod` on the encrypted Longhorn class (§9.4, §9.5) |
| `deployment.json` | one replica, `Recreate`, non-root, read-only root filesystem, no `fsGroup`, secrets projected read-only (0400), probes and `/metrics` on the private health listener |
| `service.json` | `https` only; the health listener is never a Service port |
| `networkpolicy.json` | default deny; 8443 from the gateway only; 8081 from `monitoring` only; egress DNS, Traefik and public 443 |
| `certificate.json` | cert-manager TLS for the backend host |
| `airs-mcp.toml` | configuration template: every identifier is a placeholder (runbook step 1) |
| `kustomization.yaml` | the resources and config map; the image digest is a placeholder until runbook step 2 |
| `Dockerfile` | the production image (built and pushed by the owner) |
