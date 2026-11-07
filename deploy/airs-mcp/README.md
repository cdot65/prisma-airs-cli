# deploy/airs-mcp: GitOps artifacts for the optional runtime MCP service (M8.1)

- **Not applied by agents.** Nothing here has been applied, built or pushed by an agent.
  The owner deploys it by following docs/RUNTIME_MCP_DEPLOYMENT_RUNBOOK.md.
- **Format.** The manifests are JSON, which is valid YAML.
- **Validation.** `cargo test -p airs-mcp --test deploy_manifests` checks them statically
  (no cluster contact):
  - the hardened singleton (every container's security context);
  - secret delivery, storage, probes, metrics, the complete network rules and exposure;
  - the certificate against the configured hostnames;
  - the configuration template, through the service's own strict parser.

  Each security regression applied alone to a copy is refused (the mutation test). To
  validate your filled set before deploying, with its pinned image digest required:

  ```bash
  AIRS_MCP_DEPLOY_DIR=path/to/your/set AIRS_MCP_REQUIRE_DEPLOYABLE=1 \
    cargo test -p airs-mcp --test deploy_manifests
  ```

  A run that validated it prints `DEPLOYABLE: sha256:…`; without the variable the
  test prints `DEPLOYABLE SKIPPED`.
- **Secret delivery.** Projected Secret files are root-owned, and the pod has no
  `fsGroup` (contract §9.3), so the service user (UID 65532) cannot read them in place.
  The `secrets` init container is the only reader of the raw Secret volumes
  (`tls-source`, `airs-secrets-source`, both 0400 and read-only). It runs as root with
  CHOWN alone, copies each file with umask 0377 into the in-memory `secrets` volume
  (`emptyDir`, medium `Memory`, 1 Mi), and gives it to 65532 in this order:
  1. chown every copied file, while its directory is still root-owned and 0700;
  2. chmod the two directories to 0500;
  3. chown the two directories last.

  Root with CHOWN alone has no DAC override, so it cannot search a directory once that
  directory belongs to 65532 at 0500. No command after step 3 names a file inside one
  (M8 review 2 finding 1). The order is checked statically, and a test runs the script
  with each directory hand-off modelled as a loss of access, which the review-2 order
  fails. The service mounts only that volume, read-only, through `subPath`s at the
  configured paths (`/etc/airs-mcp/tls`, `/run/airs-mcp/secrets`). Nothing is written
  to disk.

  **Remains for the owner's step-6 live proof:** that the init container completes as
  real root with CHOWN only in the pod; that `stat` in its log shows `65532:65532 500`
  for both directories; and that the service starts with TLS (the files were readable
  as 65532). The tests use the current user, not two real UIDs.
- **Rotation.** The copies are taken once at pod start. After you change a Secret
  (cert-manager renewal of `airs-mcp-tls`, or new AIRS credentials), restart the pod:
  `kubectl -n airs-mcp rollout restart deployment/airs-mcp`. `Recreate` stops the old
  pod before the new one copies the new values. Certificate renewal leaves well over
  two weeks of validity, so a routine restart within that window is enough.

| File | What |
|---|---|
| `namespace.json` | `airs-mcp` namespace, Pod Security `baseline` (the CHOWN-only init container) |
| `pvc.json` | 100 Gi `ReadWriteOncePod` on the encrypted Longhorn class (§9.4, §9.5) |
| `deployment.json` | one replica, `Recreate`, non-root, read-only root filesystem, no `fsGroup`, secrets copied by the `secrets` init container into a 65532-owned in-memory volume (0400), probes and `/metrics` on the private health listener |
| `service.json` | `https` only; the health listener is never a Service port |
| `networkpolicy.json` | default deny; 8443 from the gateway only; 8081 from `monitoring` only; egress DNS, Traefik and public 443 |
| `certificate.json` | cert-manager TLS for the backend host |
| `airs-mcp.toml` | configuration template: every identifier is a placeholder (runbook step 1) |
| `backup-cronjob.json` | the daily coordinated backup (§9.4): an orchestrator CronJob (03:00 America/Chicago, `Forbid`) stops the service, runs `control-backup-set` then `control-backup-verify --latest-in` in a Job from the suspended `airs-mcp-backup-set` template, waits for it, and always scales the service back to 1; minimal RBAC and an API-server-only egress policy for the orchestrator |
| `kustomization.yaml` | the resources and config map; both image digests (airs-mcp and the orchestrator's kubectl) are placeholders until runbook step 2 |
| `Dockerfile` | the production image (built and pushed by the owner) |

## Daily coordinated backup (`backup-cronjob.json`)

Contract §9.4 requires a coordinated, SQLite-consistent set under exclusive maintenance
every day, before the off-volume Longhorn backup. The data PVC is `ReadWriteOncePod`, so
a pod that mounts it cannot even be scheduled while the service pod runs, and a single
backup pod cannot stop the service first. The set is therefore taken by two resources:

- **`airs-mcp-backup`** (the orchestrator CronJob). At 03:00 America/Chicago it
  scales `deployment/airs-mcp` to 0, waits up to 300 s until the service pod is gone,
  creates a Job from the suspended template, and polls the Job's own conditions until
  `Complete=True` (success) or `Failed=True` (the set failed, or the Job's deadline
  ended it), so a failed set is noticed at once rather than at a timeout. On every exit
  (success, failure, SIGTERM, or the orchestrator's own deadline), a shell `trap` first
  waits until every set-Job pod is **terminal** (`Succeeded` or `Failed`), then scales
  the Deployment back to 1, then polls until it is available (up to 300 s). A failed set
  fails the orchestrator Job (`backoffLimit: 0`). Every `kubectl` call carries
  `--request-timeout=30s`.
  - *Volume release.* A completed Job keeps its pod (`ttlSecondsAfterFinished`, 14 days),
    so the trap never waits for the pod to be *deleted*: it waits for its phase. A
    terminated pod no longer holds the `ReadWriteOncePod` attachment, and the
    scheduler ignores terminal pods when it places the service pod again.
  - *Budgets* (checked statically by `deploy_manifests.rs`, and exercised by its
    orchestrator test against stub `kubectl`):
    - the set Job's own `activeDeadlineSeconds: 1200` ends it before the orchestrator's
      poll gives up (1230 s) and before the trap's release wait gives up (1260 s);
    - the **full path** is bounded by stop 300 + poll 1230 + release 1260 + ready 300 +
      margin 300 = 3390 s, strictly below the orchestrator's `activeDeadlineSeconds:
      3600`;
    - the **termination path** (the trap alone, after a SIGTERM or the deadline) is
      bounded by release 1260 + ready 300 + four bounded API calls + margin = 1980 s,
      which is the pod's `terminationGracePeriodSeconds`, so the kubelet lets the trap
      finish.
  - *Downtime* is the length of the set plus the restart. There is no fixed restoration
    time: the service is down from 03:00 until `service_restored` is logged, which on a
    normal day is a few minutes; the budgets above are worst cases. Schedule the Longhorn
    job group after the observed completion time (runbook §9a), not before it, and gate
    the day on both events.
- **`airs-mcp-backup-set`** (suspended; a Job template only). One pod with the service's
  hardening (non-root 65532, read-only root, no capabilities, no `fsGroup`), no
  Secret and no API token. Its init container runs
  `airs-mcp control-backup-set --dir /data/airs/backups --keep 3`: the new set is
  verified, then complete sets beyond the newest 3 and older interrupted runs (a set
  directory with **no** manifest: creation writes the manifest last) are removed.
  Retention refuses, removing nothing, if any set has a manifest that is not exactly a
  complete one of this format: unreadable, corrupt, foreign, a missing field, or
  `complete` missing, `null`, `false` or not a boolean. A damaged published manifest is
  never mistaken for an interrupted run. Its container then runs
  `airs-mcp control-backup-verify --latest-in /data/airs/backups` over the data mounted
  read-only. The pod is labelled `airs-mcp-backup`, so the service's network policies
  and the orchestrator's wait never select it, and the namespace default-deny leaves it
  no network at all.
- **RBAC** (`Role airs-mcp-backup`): get and scale only `deployment/airs-mcp`, get only
  `cronjob/airs-mcp-backup-set`, create and watch Jobs, and read pods. Nothing else.
  Only the orchestrator pod mounts the ServiceAccount token.
- **Network:** `airs-mcp-backup-to-apiserver` lets only the orchestrator reach the API
  server (the control-plane VIP and nodes, 192.168.255.10–13:6443).

Pin the `kubectl` image digest in `kustomization.yaml` (runbook step 2), as for the
service image. The orchestrator logs three distinct events:
- `{"event":"backup_set_verified",...}`: the set Job completed; its set was written and
  verified;
- `{"event":"service_restored"}`: the Deployment is back at 1 and available, logged even
  after a failed set;
- `{"event":"backup_set_daily_complete",...}`: both of the above; the day's success
  marker. The orchestrator exits 0 only then.

Runbook §9 states how the Longhorn backup waits on the success marker.

**Interrupted orchestrator.** If the orchestrator pod is killed without running its trap
(node loss, an eviction past its grace period, or a SIGKILL), the service can stay at 0
replicas. Detect it: the service's readiness probe fails, and `kubectl -n airs-mcp get
deployment airs-mcp` shows `0/0`. Recover it as the owner:
1. Wait until every set-Job pod is terminal (`Succeeded` or `Failed`; a completed pod
   stays listed for 14 days and holds nothing):
   `kubectl -n airs-mcp get pods -l app.kubernetes.io/name=airs-mcp-backup`
   (the Job's `activeDeadlineSeconds` ends a stuck set within 20 minutes).
2. Then `kubectl -n airs-mcp scale deployment/airs-mcp --replicas=1`, and wait for it to
   become available.
3. Check that day's set before counting it:
   `airs-mcp control-backup-verify --latest-in /data/airs/backups` in a maintenance pod,
   with the service stopped (runbook §9a). Otherwise record the day as a gap.

The next day's run scales the service to 0 and back as usual.
