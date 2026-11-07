# airs-mcp deployment runbook (M8.3)

This runbook deploys the optional runtime MCP service and records the user-run proof
M8 needs before deployment is called accepted (plan M8 exit gate: an independent
≥9.0 (owner-set threshold) source and receipts review **plus** actual integration evidence).

- **Who runs it:** the owner. Agents prepared the artifacts and checks. They never
  apply, push, sign or contact the cluster.
- **Recording:** run `scripts/m8/user-checklist.sh` alongside these steps. It asks for
  each result and writes a 0600 evidence file. It refuses anything that looks like a
  credential.

Artifacts: `deploy/airs-mcp/` holds:

- the namespace, Certificate, PVC, Deployment, Service and NetworkPolicies;
- the configuration template `airs-mcp.toml`;
- `kustomization.yaml` and the `Dockerfile`.

The manifests are JSON, which is valid YAML. `cargo test -p airs-mcp --test
deploy_manifests` validates them statically against contract §9.3–§9.5 and §11 and
against the service's own configuration parser.

## Before you start

- **Real data waits.** No real data is stored until steps 4 and 5 pass (contract §9.4).
- **Never a shared node.** Never power off, drain or isolate a shared talos node. The
  node-loss test (step 10) uses a **dedicated, disposable** node only.
- **Encrypted storage required.** If the encrypted StorageClass, the backup target or key
  custody cannot be established, deployment stays blocked. A retained PVC is not a
  backup.

## 1. Fill the configuration

Replace every placeholder in `deploy/airs-mcp/airs-mcp.toml`:

- the tenant;
- the Entra tenant, audience and client IDs;
- the gateway assertion issuer, audiences, workspace and organisation IDs;
- grants and receipt associations;
- the scanner name, application ID and revision;
- the management credential revision.

Keep every path, listener and bound as they are: the deployment mounts exactly those.
Then:

```sh
cargo test -p airs-mcp --test deploy_manifests
```

The test fails if the template no longer parses or if any credential file is not
projected.

## 2. Build, push and pin the image

```sh
scripts/m8/build-hashes.sh .local/m8-build        # local binaries + SHA-256 manifests
DOCKER_BUILDKIT=1 docker build --platform linux/amd64 -f deploy/airs-mcp/Dockerfile \
  -t <registry>/airs-mcp:<version> .
docker push <registry>/airs-mcp:<version>
```

1. Put the pushed digest in `deploy/airs-mcp/kustomization.yaml`
   (`images[0].digest`). The committed all-zero digest is a placeholder; the manifest
   test treats it as not deployable.
2. Record the digest and the `BIN-SHA256SUMS` of the build.

## 3. Storage class and secrets

Confirm the following exist:

- `StorageClass` `longhorn-airs-encrypted`, encrypted (dm-crypt), with its passphrase
  from Conjur `data/airs-mcp/backup/volume-passphrase` through ESO, and the owner's
  offline escrow copy held;
- `airs-mcp-airs-credentials` (ESO, from Conjur), with keys `mgmt-client-id`,
  `mgmt-client-secret` and `scanner-monitor`;
- `airs-mcp-tls`, issued by cert-manager for `mcp-backend-airs.goshawks.io`;
- `harbor-pull`, the pull secret for the registry.

## 4. nas1 retention record (contract §9.4, open check 1)

On nas1 (DSM, read-only inspection), record:

- whether the nas2 → nas1 sync **mirrors deletions**;
- its retention or rotation for the Longhorn backup share;
- the job name and schedule.

Save a screenshot or an export of the settings and give its path to the checklist. If
nas1 keeps copies indefinitely, update §9.4's deletion bound before storing real data.

## 5. No agent-accessible R2 delete right (contract §9.4, open check 2)

From **each** agent workspace image (Coder), run:

```sh
scripts/m8/check-r2-delete-rights.sh
```

The script is read-only:

- Part 1 lists credential **names** visible in the workspace (environment, AWS
  profiles, rclone remotes, wrangler state). It never reads their values.
- Part 2 inspects any API token you paste at a hidden prompt, using only the token-verify
  and token-detail reads.
- An R2 S3 key inherits its parent token's permissions, so inspect the parent.

Narrow or revoke every credential it flags, re-run until none is flagged, and give the
evidence file to the checklist.

## 6. Deploy

Apply through your GitOps path, for example `kubectl apply -k deploy/airs-mcp`. Then
confirm:

- one pod Running, and the `Recreate` rollout never overlaps two pods;
- the PVC is Bound, `ReadWriteOncePod`, on the encrypted class;
- `kubectl port-forward deploy/airs-mcp 8081:8081` shows the probe and metrics endpoints:
  - `/livez` returns `ok`;
  - `/readyz` returns `ready`;
  - `/metrics` shows `airs_mcp_ready 1` and `airs_mcp_control_ok 1`, plus the ten job
    states and the volume, archive and control sizes.
- The pod log shows `storage_owned` and `control_recovered`, and no
  credential.

Point capacity alerts at `airs_mcp_data_volume_free_bytes` relative to
`airs_mcp_data_volume_size_bytes`: 70% and 85% used (§9.5).

## 7. Gateway sign-in and service proof

```sh
airs mcp connections set <name> --url https://mcp.goshawks.io/<path> ...   # see airs mcp connections set --help
airs mcp login <name>
airs mcp status <name>
airs runtime history status --mcp <name>
```

Record the outputs. They contain no secrets.

## 8. Claude Code proof

Connect Claude Code to the gateway MCP URL. Call `service_info` and `history_status`, and
record the results, including the service revision.

## 9. Backup marker test

1. Through the service, write a recognisable plaintext marker into the archive: for
   example, a receipt with a unique `receipt_key`.
2. Take a Longhorn backup.
3. Search the backup target for the marker, decompressing as Longhorn does. It must
   **not** be found: encrypted volumes keep ciphertext backups.
4. Record the search command and its result.

## 10. Node-loss test: dedicated, disposable node only

Plan revision 7 amendment 3; contract §9.3.

1. Add a disposable worker node that nothing else schedules onto.
2. Pin a test copy of the deployment (its own namespace and PVC) to that node.
3. Make the node unavailable the way a real loss would.
4. Record whether the volume reattached elsewhere **without** automated force-detach, how
   long it took, and whether the second pod stayed Pending until the first was gone.
5. Confirm the archive's integrity afterwards.

A negative result reopens the storage design.

## 11. Restore drill

1. Restore the latest Longhorn backup into an isolated namespace.
2. Start `airs-mcp` there with `maintenance = true`.
3. Run `airs-mcp control-*` checks offline as the contract's §9.1 maintenance commands
   describe. These cover the **control store**; the archive is checked by
   `history_status` and the integrity check.
4. Record `history_status`, the integrity result and the row counts.
5. Repeat monthly.

## Rollback

- **Bad release:** set the previous image digest and re-apply. `Recreate` stops the old pod
  first. The data subtree is never repaired or recursed by the init container.
- **Corrupt store:** do not delete it. Start in maintenance mode and use the guarded
  recovery tools: `airs runtime history recover --mcp <name>` online (the `lifecycle_preview`/`lifecycle_execute` action `recover`), or the offline `control-*` commands.
