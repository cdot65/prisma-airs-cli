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

- **Real data waits** until steps 1–3 and 6–7 pass. Steps 4–5 are optional operator
  records of your own backup policy (contract §9.4: backup targets, retention and
  deletion rules are the operator's, not a gate of this product).
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

### Grants the proofs need

The proofs in steps 7b, 8, 9b and 12 call these tools. The template's grants
(`deploy/airs-mcp/airs-mcp.toml`, and `scripts/m8/derive-config.sh`'s output) must
cover them for the caller who runs the proof:

| Proof | Tools (catalogue names) | Tier |
|---|---|---|
| 7b, 8: sign-in and service proof | `service_info`, `history_status`, `history_coverage` | reader |
| 9b: marker report | `reports_create`, `jobs_get`, `jobs_list`, `reports_list`, `reports_get`, `artifacts_read`, `artifacts_download` | operator (`reports_create`); the rest are reader |
| 12: low-space qualification | the 9b tools, plus `collection_start` and `receipts_ingest` | operator |

A grant lists tools explicitly; a tool missing from the caller's grant is refused.
Keep the Claude Code grant at reader unless its proof needs more.

## 2. Build, push and pin the image

```sh
scripts/m8/build-hashes.sh .local/m8-build        # local binaries + SHA-256 manifests
DOCKER_BUILDKIT=1 docker build --platform linux/amd64 -f deploy/airs-mcp/Dockerfile \
  -t <registry>/airs-mcp:<version> .
docker push <registry>/airs-mcp:<version>
```

1. Put the pushed digest in `deploy/airs-mcp/kustomization.yaml`
   (`images[0].digest`, the `airs-mcp` image). The committed all-zero digest is a
   placeholder; the manifest test treats it as not deployable.
2. Pin the daily backup orchestrator's `kubectl` image too (the `kubectl` entry in the
   same `images` list, `registry.cdot.io/dockerhub/bitnami/kubectl`). Resolve the tag you
   choose to its digest (`crane digest …` or `docker buildx imagetools inspect …`) and set
   `digest`. The orchestrator bounds every `kubectl` invocation with coreutils `timeout`,
   so confirm the pinned image has it:
   `podman run --rm --entrypoint timeout registry.cdot.io/dockerhub/bitnami/kubectl@sha256:<digest> --version`
   must print a coreutils version. If it does not, choose another tag or build a derived
   image; the orchestrator refuses to run without it (it exits 1 before touching the
   service, logging `orchestrator_preflight_failed`). Every image must be pinned for the
   set to be deployable:
   `AIRS_MCP_DEPLOY_DIR=deploy/airs-mcp AIRS_MCP_REQUIRE_DEPLOYABLE=1 cargo test -p airs-mcp --test deploy_manifests`
   prints `DEPLOYABLE: …` only then.
3. Record both digests and the `BIN-SHA256SUMS` of the build.

## 3. Storage class and secrets

Confirm the following exist:

- `StorageClass` `longhorn-airs-encrypted`, encrypted (dm-crypt), with its passphrase
  from Conjur `data/airs-mcp/backup/volume-passphrase` through ESO, and the owner's
  offline escrow copy held;
- `airs-mcp-airs-credentials` (ESO, from Conjur), with keys `mgmt-client-id`,
  `mgmt-client-secret` and `scanner-monitor`;
- `airs-mcp-tls`, issued by cert-manager for `mcp-backend-airs.goshawks.io`;
- `harbor-pull`, the pull secret for the registry.

## 4. Off-volume backup policy (operator record; optional)

The service stores everything in SQLite on its data volume and knows nothing about
backup targets. If your environment copies the volume or the backup sets elsewhere,
record for your own operations: where the copies go, their retention and rotation, who
can delete them, and whether a mirror propagates deletions. This is an operator record,
not an acceptance gate (contract §9.4, revised 2026-11-07); `skip` it in the checklist
with the reason "no off-volume copies" when none exist.

## 5. Agent access to backup copies (operator record; optional)

If agent workspaces in your environment can reach credentials for an off-volume backup
store, confirm none of them can delete backup objects. For a Cloudflare R2 bucket,
`scripts/m8/check-r2-delete-rights.sh INVENTORY.json` validates a sanitized inventory
offline (schema in its header: names, kinds, permission groups and scopes only, never a
value; exit 0 clean, 3 incomplete/unknown, 4 violation). Optional; `skip` with a reason
when not applicable.

## 6. Deploy

Apply through your GitOps path, for example `kubectl apply -k deploy/airs-mcp`. If a
GitOps controller with self-healing manages the Deployment (Argo CD `selfHeal`), it must
**ignore replica drift** on `deploy/airs-mcp`: the daily set job (§9a) scales it to 0 and
back while it writes and verifies the set, and a controller that heals `replicas` would
bring the service up mid-backup. Argo CD: `ignoreDifferences` on `/spec/replicas` for the
Deployment plus the `RespectIgnoreDifferences=true` sync option.

The backup orchestrator reaches the API server through the `airs-mcp-backup-to-apiserver`
policy, whose endpoints are **your overlay's value** (the base ships a TEST-NET-1
placeholder the deployable check refuses). Supply your control-plane endpoints, and if your
CNI gives node IPs a node identity that `ipBlock` rules do not match (Cilium), point the
orchestrator at a control-plane VIP that is not a node address with the
`KUBERNETES_SERVICE_HOST`/`KUBERNETES_SERVICE_PORT` env on its `orchestrate` container.
Both were found on the first live set run (M8 live evidence, 2026-11-08). Then confirm:

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

### 7a. Gateway binding evidence (plan revision 7 amendment 1; re-checked at every deployment)

The binding to this backend is the upstream Entra token (`aud` = this service's API,
`azp` = the gateway client). It is safe only while the gateway client is dedicated to
this one integration. Record each item below. Record IDs and settings only, never a
secret value.

1. **The gateway client is confidential only.** No public-client, SPA, implicit or
   federated issuance path:
   ```sh
   az ad app show --id <gateway-client-id> \
     --query "{publicClient:isFallbackPublicClient, spa:spa.redirectUris, implicit:web.implicitGrantSettings}"
   az ad app federated-credential list --id <gateway-client-id>
   ```
   Expected: `publicClient` false or null; `spa` empty; both implicit settings false;
   no federated credentials.
2. **It is the only client consented to the API.** List the API service principal's
   delegated grants and app-role assignments:
   ```sh
   az rest --method GET --url "https://graph.microsoft.com/v1.0/oauth2PermissionGrants?\$filter=resourceId eq '<api-sp-object-id>'"
   az rest --method GET --url "https://graph.microsoft.com/v1.0/servicePrincipals/<api-sp-object-id>/appRoleAssignedTo"
   ```
   Expected: every `clientId` and `principalId` is the gateway client's service
   principal.
3. **Its credential is used by this integration alone.** `az ad app credential list
   --id <gateway-client-id>` shows exactly the secret configured on this gateway
   integration. Record its `keyId` and end date, not its value. In the gateway console,
   no other integration uses that client.
4. **Nothing else targets this backend.** In the gateway console, no integration other
   than this one has the upstream URL `https://mcp-backend-airs.goshawks.io/mcp`, or
   your `[server].resource`.

**Refuse and stay blocked** if any item fails or cannot be shown. Do not sign in, and do
not store real data. Narrow the registration or the gateway configuration first, then
repeat 7a.

The same four checks apply to every other route into an `airs-mcp` backend, with that
route's own IDs: the low-space qualification (§12 step 2) has its own API registration
and its own confidential gateway client, and is checked the same way before anything
signs in to it. The production client must never serve a second integration, so a
qualification integration never reuses it.

### 7b. Sign-in and service proof

```sh
airs mcp connections set <name> --url https://mcp.goshawks.io/<path> ...   # see airs mcp connections set --help
airs mcp login <name> --open --port 53682
airs mcp status <name>
airs runtime history status --mcp <name>
```

`--port 53682` is required: the gateway registered the `airs-cli` public client with the
exact loopback redirect `http://127.0.0.1:53682/callback` (the `airs-admin-cli` client
uses 53683) and matches it exactly, so the default random port is refused with
`invalid_request: Invalid redirect_uri`. Free the port rather than changing it.

Record the outputs. They contain no secrets.

## 8. Claude Code proof

Connect Claude Code to the gateway MCP URL. Call `service_info` and `history_status`, and
record the results, including the service revision.

## 9. Backup marker test and the backup set

### 9a. The daily backup (contract §9.4)

Contract §9.4 defines the daily **coordinated application set**, taken under exclusive
maintenance; it is the service's backup content. Any off-volume copy of the volume or the
sets (and when it runs) is the **operator's** policy: in the reference deployment the
operator's daily Longhorn backup runs after the set. A crash-consistent snapshot of the
running volume is not a substitute for the set.

**Schedule (America/Chicago):**

| Time | What | Where defined |
|---|---|---|
| 03:00 | The daily set job: Deployment scaled to 0 → `control-backup-set` → `control-backup-verify` → Deployment scaled to 1 → success marker. Normally a few minutes; bounded at 65 min (`activeDeadlineSeconds: 3900`), with the restore trap allowed 33 min (`terminationGracePeriodSeconds: 1980`) if that deadline hits | `deploy/airs-mcp/backup-cronjob.json` |
| 03:15 | The operator's volume backup, if configured (the reference cluster: a Longhorn recurring job), carries the new set and the stopped-then-restarted volume off-volume | the operator's backup jobs |

**The daily set job** runs in the `airs-mcp` namespace with a ServiceAccount that may only
scale `deploy/airs-mcp`. It must:
1. Scale `deploy/airs-mcp` to 0, and wait (up to 300 s) until the service pod is gone
   and the RWOP volume is released.
2. Run, in a maintenance pod (below) with the same image digest:
   `airs-mcp control-backup-set --config /etc/airs-mcp/config/airs-mcp.toml`.
   - Under the data subtree's exclusive owner lock, it writes a new
     `/data/airs/backups/<id>/` (0700; files 0600; everything fsynced), containing:
     - `VACUUM INTO` copies of the control store and every archive store;
     - the artifact list (every published report version's files, from the control
       store);
     - `manifest.json`, written last: `{format: "airs-mcp-backup/1", created_at,
       epoch, stores[{name, schema, sqlite_version, bytes, sha256, rows_digest}],
       artifacts[{id, sha256, bytes}], complete: true}`.
   - It prints `{"event":"backup_set","set":"<id>",…}`.
   - It refuses while the service holds the data directory ("stop it first").
3. Run `airs-mcp control-backup-verify --set /data/airs/backups/<id>`. It re-hashes
   every store copy, recomputes `rows_digest`, checks the manifest metadata, and
   refuses an incomplete manifest, a mismatched or missing store, or an unlisted file.
   It prints `{"event":"backup_set_verified",…}`.
4. Scale `deploy/airs-mcp` back to 1, **whatever** happened in steps 2–3, once every
   set pod is terminal (`Succeeded` or `Failed`; up to 1260 s of elapsed time, request
   time included and every `kubectl` invocation cut at the time left, which covers the set
   Job's own 1200 s deadline). A completed pod stays
   listed for 14 days and holds nothing.
5. Only after a verified set, write that day's success marker. The job's log carries
   the `backup_set` and `backup_set_verified` lines.

**Downtime window.** The service is stopped from about 03:00 until the set is verified,
normally a few minutes and dominated by `VACUUM INTO` of the archive. Measure it on the
first runs (step 12, measured capacity). The worst case is the orchestrator's budget:
scale-down 35 + stop 305 + create 35 + poll 1265 + release 1265 + restore 365 + margin
300 = 3570 s of elapsed time (every phase's API requests included; each `kubectl`
invocation is cut at the lesser of 30 s and the time left in its phase, with 5 s to
force-terminate it, and loop sleeps never pass the phase deadline, so a phase overruns by
at most those 5 s), inside its 3900 s deadline; and if the deadline or a SIGTERM ends it,
the trap starts within 35 s (the longest any invocation, even a hung one, can keep the
shell from running it) and needs at most 35 + release 1265 + restore 365 + margin 300 =
1965 s, inside its 1980 s of grace, to restore the service. On such a day the 03:15 Longhorn backup runs before the set is
verified, so the success gate below records no coordinated backup for it. While it is stopped, agents and the CLI get
connection errors, and no collection runs. The quarter-hour schedule coalesces the
missed tick (contract §6.1).

**Success gate.** A day's backup counts only if that day's set verified **and** the
03:15 Longhorn backup completed after it. Check both every day: the job's success
marker and log lines, and the Longhorn backup's completion time. If the set job failed
or did not finish before 03:15:
- that day has **no** coordinated backup. The off-volume copy still holds the previous
  verified sets and a crash-consistent image of the volume, but neither replaces the
  missing set;
- alert, fix the cause, and run the job by hand (`kubectl -n airs-mcp create job
  --from=cronjob/airs-mcp-backup airs-mcp-backup-manual-<date>`), then trigger a
  Longhorn backup of the volume after it;
- record the gap. The RPO for that day is the previous verified set.

**RPO.** The newest verified set that has been carried off-volume, normally under 24 h.

**Retention.**
- *On the volume:* keep the newest **3** verified sets under `/data/airs/backups/`.
  Remove an older on-volume set only after a newer set has verified. Sets are part
  of the §9.5 capacity budget (about 1× the databases each).
- *Off the volume:* the operator's policy (optional step 4 record; contract §9.4).
  The reference environment's own practice is that no agent deletes a backup or a
  mirrored copy; that is operator policy, not a requirement of this product.

**Before an upgrade,** run the job by hand, as above, and check its verified line
before changing the image.

The online `backup` lifecycle action answers `maintenance_required`: the set is taken
only offline. Restore uses a **different**, control-only set (`control-backup`, C9),
made from the restored store during recovery (step 11). `control-restore` reads only
that control-only format, not a coordinated set.

**The maintenance pod.** `control-*` commands refuse while the service holds the data
directory ("the service holds the data directory; stop it first"), so run them only
with the Deployment at 0 replicas. The daily set job uses the same shape. Use a pod in
the same namespace with:
- the same image digest, `securityContext` (UID/GID 65532, no `fsGroup`) and
  `airs-mcp-config` ConfigMap;
- the data PVC (RWOP: only one pod at a time);
- `command: ["/usr/local/bin/airs-mcp"]` and the arguments shown.

It needs no Secrets: configuration loading only parses the file paths.

### 9b. Marker test

The marker must be in a field the service stores as written. A receipt key does
**not** qualify: the archive stores a hash of it, so its absence from a backup proves
nothing. Use a report title instead. It is stored as given in the control store's
`reports.title` column and rendered into the bundle's `report.html`.

1. **Create the marker:**
   ```sh
   M="AIRSMCP-MARKER-$(uuidgen)"
   airs runtime report --mcp <name> --title "$M" --interval 1 --unit days --output json
   ```
2. **Prove it is in the stored data (positive control):**
   ```sh
   airs runtime history reports --mcp <name> --output json          # lists the title
   airs runtime history download --mcp <name> --version-id <v1_…> --output-dir ./marker-check
   grep -c "$M" ./marker-check/report.html                          # must be >= 1
   ```
   If this is 0, stop: the test cannot discriminate.
3. **Back it up.** Run the daily set job by hand (9a) and check its
   `backup_set_verified` line, then take a Longhorn backup of the volume after it.
   Record the set ID and the backup name.
4. **Show positive encryption evidence.** All of the following must hold:
   - `kubectl -n longhorn-system get volumes.longhorn.io <volume> -o jsonpath='{.spec.encrypted}'`
     is `true`, and the StorageClass has `encrypted: "true"` with its passphrase
     Secret reference.
   - On the backup target, the backup's blocks, decompressed as Longhorn does (the
     backup's `compressionMethod`), show:
     - `grep -c "$M"` is 0;
     - no `SQLite format 3` header and no ext4 directory names;
     - about 8 bits/byte of entropy across written blocks;
     - only the LUKS header is structured (`LUKS\xba\xbe` at the volume start).

     These are M0.6's P8 checks. A missing marker counts only together with these
     checks.
5. **Wrong key refused.** Restore the backup into the isolated namespace (step 11)
   through a StorageClass whose passphrase Secret holds a **wrong** value. The volume
   must fail to open: the attach or mount fails with a cryptsetup or LUKS error in the
   Longhorn or CSI events. Record the event.
6. **Correct key restores the marker.** Restore it again with the correct class. With
   no `airs-mcp` pod running, mount the restored PVC **read-only** in a debug pod: UID
   65532, no `fsGroup`, `automountServiceAccountToken: false`. Then run:
   ```sh
   grep -c "$M" /data/airs/control/control.sqlite3      # >= 1
   grep -rlc "$M" /data/airs/reports                    # the bundle's report.html
   ```
   Delete the debug pod and the restored PVC afterwards.

Record every command and its result. Steps 2, 4, 5 and 6 together make the evidence;
any one alone does not.

## 10. Node-loss test: dedicated, disposable node only

Plan revision 7 amendment 3; contract §9.3. **Never use a shared talos node.** Every
node named here is a temporary worker created for this test and removed after it.

1. **Prepare.** Add two disposable worker nodes, A and B. Label both
   `airs-mcp-nodeloss=true`, and taint both so only the test schedules there. Cordon
   B. Deploy a test copy in its own namespace with:
   - its own encrypted RWOP PVC;
   - a required nodeAffinity on `airs-mcp-nodeloss=true` and the taint's toleration;
   - the production manifests otherwise, and egress denied (no AIRS credentials
     needed).

   It schedules on A. Record:
   - `kubectl get pod -o wide`;
   - `kubectl get volumeattachments`;
   - the Longhorn volume's attached node and state (`kubectl -n longhorn-system get
     volumes.longhorn.io <vol> -o wide`);
   - the pod's `storage_owned` log line (owner UUID, node).
2. **Lose A.** Power A off at its hypervisor, as a real loss would. Do not drain it
   first. Then `kubectl uncordon B`.
3. **Observe while A's state is uncertain.** Every 30 s for at least 10 minutes,
   record:
   - the pod phases (the old pod goes `Terminating` or `Unknown`);
   - whether a replacement pod exists and its phase;
   - `kubectl get volumeattachments`;
   - the Longhorn volume's state and attached node.

   The expected safe result: no second `airs-mcp` pod ever becomes Running, and the
   volume stays attached to A or detaching, never attached to two nodes. **No
   force-detach and no force pod deletion** while A's power state is unproven.
4. **Fence the old writer, then reacquire.** Only after the hypervisor or BMC shows A
   **powered off**:
   - record that evidence: a screenshot or CLI output with a timestamp;
   - run `kubectl delete node A`. That removes A's pods and VolumeAttachments, so the
     Recreate Deployment's replacement can schedule on B.

   Record:
   - when the replacement became Running on B;
   - the VolumeAttachment now naming B;
   - the new `storage_owned` line, which must show a **new** owner UUID and node B;
   - that `/readyz` returns `ready` and `airs_mcp_control_ok 1`.
5. **Confirm integrity.** After reacquisition, record the `control_recovered` line and
   the archive and control byte metrics against the values from step 1.
6. **Clean up.** Delete the test namespace and remove both disposable nodes.

A dual mount, a second Running pod, or reacquisition without fencing evidence reopens
the storage design (§9.3).

## 11. Restore drill

Monthly, in an isolated namespace (`airs-mcp-drill`), on a copy. Do **not** reuse the
production namespace, gateway route or credentials. The backup itself is never modified:
the restored PVC is a copy, so the original stays on the target.

1. **Isolate.** Create `airs-mcp-drill` with:
   - a default-deny NetworkPolicy for ingress and egress;
   - the production config as the `airs-mcp-config` ConfigMap (the tenant must match:
     `control-restore` checks it);
   - a throwaway self-signed TLS Secret;
   - placeholder AIRS secret files that are not credentials.

   Apply the Deployment with **`replicas: 0`**.
2. **Restore the volume.** Restore a Longhorn backup from a day whose set verified
   (step 9a's success gate) into a new encrypted PVC in `airs-mcp-drill`. Then check the
   coordinated set it carries, read-only, in a maintenance pod:
   `airs-mcp control-backup-verify --set /data/airs/backups/<id>`. It must print
   `backup_set_verified` for the restored copy too. Record the line.
3. **Re-key the control store (contract C10/C11).** A restored volume carries the
   *old* control epoch. It must not serve under it, because keys issued after the
   backup point would be mistaken for unknown ones. Run each command in a maintenance
   pod (step 9a) with the Deployment at 0, in this order:
   1. `airs-mcp control-backup --config /etc/airs-mcp/config/airs-mcp.toml --dir /data/airs/backups/drill-<date>`.
      This makes a **control-only** set (C9) from the restored store; recovery runs on
      open (C12). `--dir` must be a new directory. Record the `control_backup` line.
      `control-restore` reads only this control-only format. Never pass it a
      coordinated `airs-mcp-backup/1` set from `control-backup-set`: those are for
      content verification (step 2) and the off-volume copy.
   2. In a debug pod (UID 65532, no `fsGroup`), move the restored store aside, keeping
      it:
      `mkdir -m 0700 /data/airs/control-preserved-<date> && mv /data/airs/control/control.sqlite3* /data/airs/control-preserved-<date>/`.
      Leave `control/seed.json` in place.
   3. `airs-mcp control-restore --config /etc/airs-mcp/config/airs-mcp.toml --set /data/airs/backups/drill-<date>`.
      This prints `{"event":"control_restored","pending_epoch":"<hex8>"}`. The restored
      store keeps job states with claims released, marks open reservations
      `uncertain`, and enters recovery quarantine.
   4. `airs-mcp control-activate-epoch --config /etc/airs-mcp/config/airs-mcp.toml --epoch <hex8>`.
      This prints `control_epoch_activated`.
   5. **Do not run `control-reconcile` in the drill.** The quarantine keeps the worker
      from claiming anything, holds every plane for at least 24 h and disables every
      schedule. Together with the egress deny and the placeholder secrets, the drill
      copy cannot send to AIRS.
4. **Start under control.** Scale the drill Deployment to 1 and record:
   - `storage_owned` and `control_recovered` in the log;
   - through `kubectl -n airs-mcp-drill port-forward deploy/airs-mcp 8081:8081`,
     `/readyz` = `ready` and `/metrics` with `airs_mcp_control_ok 1`;
   - `airs_mcp_archive_bytes` and `airs_mcp_control_bytes` against the values recorded
     when the backup was taken;
   - the job-state series, which should match the restored store, now released and
     quarantined.
5. **Verify content.** Scale back to 0, then run step 9b's marker grep on the drill PVC.
   An authenticated `history_status` against the drill copy needs a gateway route to
   it, and this runbook does not create one. Record that the check was not made.
6. **Clean up.** Delete `airs-mcp-drill`.

**Real recovery** in production uses the same steps 2–3.4 (verify the carried set, then
make the control-only set and restore from it). Before
`control-reconcile`, the Administrator reviews the restored jobs (`airs runtime jobs
list --mcp <name>`) and confirms that no other copy of the service wrote after the
backup point. They also accept that unknown sends stay charged (contract C11). Then run
`airs-mcp control-reconcile --config /etc/airs-mcp/config/airs-mcp.toml` with the
service stopped; it prints `control_reconciled`. The holds it placed stay in force: the
cooldowns expire, and schedules are re-enabled explicitly with `airs runtime schedules
set --mcp <name> ... --enabled true`.

## 12. Low-space recovery and measured capacity (§9.5)

**What the service does when space runs low.** The disk reserve is
`2 × (control + archive bytes) + 2 × 64 MiB + outstanding upload reservations + 64 MiB`
(contract §6, §9.5). The service measures it against the free bytes `statvfs` reports
on the data volume:
- Below the reserve plus 512 MiB, **publication pauses**. Report and revision jobs end
  their run `blocked_policy` without staging anything, and `transfer_upload_begin` and
  `transfer_upload_chunk` answer `storage_unavailable`. Collection continues.
- Below the reserve itself, **collection pauses too**. `collection_start` and
  enrichment jobs are `blocked_policy`, and `receipts_ingest` answers
  `storage_unavailable`.
- Nothing is written by a paused operation. The worker's policy pass requeues a paused
  job once space is freed, within its original deadline (contract §10).
- Lifecycle actions are classified by what they do. Actions that free space (prune,
  compaction) still run, so space can be recovered; compaction still refuses unless the
  free space is at least twice the archive. Actions that allocate space (export,
  import) pause with publication.
- **An unmeasurable volume fails closed.** If free space, the database sizes or the
  upload reservations cannot be measured, publication and collection both pause until
  a measurement succeeds. The metrics then report the reserve as unavailable, rather
  than reading it as zero.
- The guarded recovery never deletes or recreates a store (C12).

Watch `airs_mcp_storage_reserve_bytes` and
`airs_mcp_storage_low_space{level="publication"|"collection"}` (1 while paused), with
the 70% and 85% alerts (step 6), and act before publication pauses.

**Recovery procedure (production):**
1. When `airs_mcp_data_volume_free_bytes` falls below 15% of
   `airs_mcp_data_volume_size_bytes`, expand the PVC online:
   `kubectl -n airs-mcp patch pvc <data-pvc> -p '{"spec":{"resources":{"requests":{"storage":"<new>Gi"}}}}'`.
   Longhorn supports online expansion.
2. Confirm the new size in `airs_mcp_data_volume_size_bytes`, and that `/readyz` is
   `ready`.
3. If writes had already failed, restart the pod (`Recreate`). Record `control_recovered`
   and `airs_mcp_control_ok 1`.

**Qualification on the actual storage class.** This is a separate, isolated deployment.
It is **not** the restore drill: the drill's copy is deliberately left quarantined and
has no route, so it cannot run a job. Never run this in production.

1. **A qualification namespace, `airs-mcp-qual`, with:**
   - a default-deny NetworkPolicy. Ingress is allowed only from the gateway pods to the
     TLS port. Egress is allowed only to DNS, the Entra JWKS/metadata endpoint and the
     gateway's assertion JWKS: what token verification needs. **No egress to any AIRS
     endpoint**, so nothing can reach production AIRS.
   - a copy of the production config, changed exactly as the table below says: its own
     backend hostname (`mcp-backend-airs-qual.goshawks.io`), its own API audience and its
     own gateway client (step 2), and the step-1 grants for your caller. The assertion
     section is unchanged (the same gateway signs the assertion). A matching Certificate.
   - placeholder AIRS secret files that are not credentials.
   - a deliberately small encrypted PVC, below the reserve plus 512 MiB once filled
     (see below).
   - the `data-subtree` init container creates, beside `/data/airs`, a second
     65532-owned directory for the filler (step 5). This is the **only** manifest
     difference from production: the qualification overlay patches the init command to
     `test -d /data/airs || { mkdir -m 0700 /data/airs && chown 65532:65532 /data/airs; }; test -d /data/qual-filler || { mkdir -m 0700 /data/qual-filler && chown 65532:65532 /data/qual-filler; }; stat -c '%u:%g %a' /data /data/airs /data/qual-filler`.
     `/data` itself stays root-owned 0755; no ancestor changes. The production manifests
     never create `/data/qual-filler`.

   | Setting | Production | Qualification |
   | --- | --- | --- |
   | `[server].allowed_hosts`, `[server].resource` | `mcp-backend-airs.goshawks.io` | `mcp-backend-airs-qual.goshawks.io` |
   | `[server].scopes_supported` | `https://mcp-backend-airs.goshawks.io/mcp/AirsHistory.*` | `https://mcp-backend-airs-qual.goshawks.io/mcp/AirsHistory.*` |
   | `[upstream].audiences` | the production API's application (client) ID | the **qualification API's** application (client) ID (step 2.1) |
   | `[upstream].allowed_client_ids` | the production gateway client | the **qualification gateway client** (step 2.2) |
   | `[upstream]` issuer, `tenant_id`, `jwks_url`, `owner_object_ids` | — | unchanged |
   | `[assertion]`, `[assertion.claims]` | — | unchanged (`workspace_ids` may name a dedicated qualification workspace) |
   | `[service]`, `[airs]`, `[[airs.scanners]]` | — | unchanged, but the secret files are placeholders and egress to AIRS is denied |
   | `[[grants]]`, `[[receipt_associations]]` | — | unchanged (the caller's subject and the public client IDs are the same) |
2. **An authenticated route with its own identities.** The production gateway client
   must serve exactly one integration (§7a item 3), and the production API audience is
   the production backend's binding, so the qualification route gets its own API
   registration and its own confidential gateway client. Record IDs only, never a
   secret value.
   1. **The qualification API** (the upstream token's audience). Copy the production
      API's scope definitions with new scope IDs, under the qualification App ID URI:
      ```sh
      az ad app create --display-name airs-mcp-qual --sign-in-audience AzureADMyOrg \
        --identifier-uris https://mcp-backend-airs-qual.goshawks.io/mcp --query appId -o tsv
      az ad app show --id <prod-api-app-id> --query api > qual-api.json
      # give each oauth2PermissionScopes[*].id a new GUID (uuidgen); keep the three scope names
      az ad app update --id <qual-api-app-id> --set api=@qual-api.json
      az ad sp create --id <qual-api-app-id> --query id -o tsv   # the qualification API's SP object ID
      ```
      Put `<qual-api-app-id>` in the qualification config's `[upstream].audiences`.
   2. **The qualification gateway client** (the token's `azp`), confidential only:
      ```sh
      az ad app create --display-name airs-mcp-gateway-qual --sign-in-audience AzureADMyOrg --query appId -o tsv
      az ad sp create --id <gw-qual-client-id>
      az ad app permission add --id <gw-qual-client-id> --api <qual-api-app-id> \
        --api-permissions <read-scope-id>=Scope <operate-scope-id>=Scope <admin-scope-id>=Scope
      az ad app permission admin-consent --id <gw-qual-client-id>
      ```
      Put `<gw-qual-client-id>` in `[upstream].allowed_client_ids`.

      **Its secret is an owner-only handoff** (M8 review 5): the value exists in your
      terminal and the gateway's secret store, and nowhere else. Not in an agent
      session, a log, the checklist's evidence file, or a command line of this
      repository's CLI.
      1. In your own terminal, with history off for the command (`set +o history`, or
         `HISTCONTROL=ignorespace` and a leading space), create the credential and
         print only its value:
         ```sh
         az ad app credential reset --id <gw-qual-client-id> --append --years 1 \
           --display-name airs-qual-gateway --query password -o tsv
         ```
         `az ad app credential reset` returns `appId`, `password` and `tenant`; it has
         no `keyId` field, so `--query keyId` prints nothing. The `password` is the
         one-time value.
      2. In the gateway console, create the secret for the qualification integration
         (the same way the production integration's secret was created) and paste the
         value directly into it. Then clear the terminal (`clear`, and
         `set -o history` again). The gateway secret is referenced by name in
         `--secret-mappings` below; the value never passes through the CLI.
      3. Record the credential's metadata only, from the listing, which cannot return a
         value (`az ad app credential list`: "the content of the password … is not
         retrievable"):
         ```sh
         az ad app credential list --id <gw-qual-client-id> \
           --query "[].{keyId:keyId,displayName:displayName,end:endDateTime}" -o table
         ```
         The checklist's evidence file holds the `keyId` and the end date, nothing else.
   3. **The gateway integration**, from the committed M0.6 template with the
      qualification values (the production integration is left untouched):
      ```sh
      airs gateway mcp integrations create \
        --file prototypes/runtime-mcp/deploy/m0.6/gateway/integration.template.json \
        --name airs-qual --slug airs-qual --transport http \
        --url https://mcp-backend-airs-qual.goshawks.io/mcp --auth-type oauth_auto \
        --set-string configurations.client_id=<gw-qual-client-id> \
        --set-string configurations.audience=<qual-api-app-id> \
        --secret-mappings '<the template's mapping, naming the gateway secret created in 2.2>' \
        --output json
      ```
      `airs gateway mcp integrations create --help` lists every flag; override the
      template's own field names with `--set-string`. `--configurations` takes inline
      JSON only (it would put a value into the process arguments and shell history), so
      the secret is never passed through it: the integration reaches its secret only by
      the mapping's name. Record the integration UUID for cleanup. Enable identity
      forwarding as for production.
   4. **Routing.** Add a CoreDNS rewrite for `mcp-backend-airs-qual.goshawks.io` to
      `airs-mcp.airs-mcp-qual.svc.cluster.local`, and that hostname to the gateway's
      `TRUSTED_CUSTOM_HOSTS` (the same shape as production; `scripts/m8/derive-config.sh`
      prints the snippets). No public DNS record: cert-manager proves the name with its
      own temporary DNS-01 TXT record.
   5. **Repeat §7a** for the qualification route before signing in: items 1–4 with
      `<gw-qual-client-id>`, the qualification API's SP object ID, the `keyId` recorded
      in 2.2 and the URL `https://mcp-backend-airs-qual.goshawks.io/mcp`. Refuse and stay
      blocked if any item fails.
   6. Then `airs mcp connections set qual …` and `airs mcp login qual`.

   Everything in this step is removed in step 8.
3. **Seed it.** Either start fresh (an empty archive, then
   `airs runtime receipts upload --mcp qual --from-bulk-state <checkpoint>` with a
   synthetic checkpoint), or restore a copy as in step 11.2–11.3.4 and then **reconcile
   deliberately**:
   `airs-mcp control-reconcile --config /etc/airs-mcp/config/airs-mcp.toml` with the
   Deployment at 0. This releases the quarantine so the worker runs. That is
   acceptable here only because egress to AIRS is denied and the secrets are
   placeholders. Leave schedules disabled.
4. **Start it at 1 replica.** Check `airs runtime history status --mcp qual`, and
   `/readyz` = `ready` through `kubectl -n airs-mcp-qual port-forward deploy/airs-mcp 8081:8081`.
5. **Fill the volume.** The filler lives in `/data/qual-filler/`, the sibling of
   `/data/airs` that the qualification init container created (step 1). It is a sibling
   for two reasons, both in the code:
   - `/data` is root-owned 0755 (contract §9.3), so UID 65532 cannot create anything
     directly in it. The init container (root, `CAP_CHOWN` only) creates the directory
     and gives it to 65532, as it does `/data/airs`.
   - Admission checks only the data directory and its ancestors, never its siblings
     (`storage::admit`), and the disk reserve sums the files of the control directory and
     the history root (`metrics::store_bytes_checked`, called from `Storage::space`). A
     filler *inside* `/data/airs` would be counted as database bytes and inflate the
     reserve. A sibling is invisible to both, so only the volume's free space changes,
     which is what the qualification measures.

   With the Deployment at 0 (the PVC is ReadWriteOncePod, so only one pod may mount it),
   run a `filler` debug pod from the pinned busybox image of `deployment.json`, as UID
   65532 with no `fsGroup` and no service-account token, mounting the qualification PVC
   at `/data`, and fill until about 1% remains. The helper is BusyBox, so the recipe uses
   only BusyBox applets and POSIX options: `df -P` (its 1024-byte-block output; GNU's
   `--output=` is rejected), `dd`, `stat -c`, `rm`. BusyBox 1.37 also ships `fallocate`,
   which is faster; `dd` is the fallback when it is missing or refused by the filesystem.
   ```sh
   FREE_KIB=$(kubectl -n airs-mcp-qual exec filler -- sh -c "df -Pk /data/qual-filler | awk 'NR==2{print \$4}'")
   case "$FREE_KIB" in ''|*[!0-9]*|0) echo "refuse: no free-space figure ($FREE_KIB)"; exit 1;; esac
   FILL_KIB=$(( FREE_KIB - FREE_KIB / 100 ))
   kubectl -n airs-mcp-qual exec filler -- sh -c \
     "fallocate -l $(( FILL_KIB * 1024 )) /data/qual-filler/fill 2>/dev/null \
      || dd if=/dev/zero of=/data/qual-filler/fill bs=1024 count=$FILL_KIB"
   kubectl -n airs-mcp-qual exec filler -- stat -c '%u:%g %a %s' /data/qual-filler /data/qual-filler/fill
   ```
   `df -P` reports the filesystem of its argument, so the figure is the data volume's
   free space whatever the mount path. The `case` refuses an empty, non-numeric or zero
   figure before any arithmetic. Expected: `65532:65532 700` for the directory, and a
   file of about `FILL_KIB` KiB. Delete the `filler` pod before step 6, or the service pod
   cannot attach the volume.
6. **Start the service and record:**
   - whether it starts (`storage_owned`, `control_recovered`) or refuses
     (`storage_refused` or `control_refused`, with the data intact);
   - `/readyz`;
   - the volume metrics, `airs_mcp_storage_reserve_bytes`, and both
     `airs_mcp_storage_low_space` series. Both must be 1 at about 1% free on a volume
     smaller than the reserve plus 512 MiB.
   - that `airs runtime report --mcp qual --interval 1 --unit days --no-wait` leaves the
     job `blocked_policy` (`airs runtime jobs get --mcp qual <job>`), not failed, and
     that `airs runtime receipts upload --mcp qual …` reports `storage_unavailable`.
7. **Free the space, without a restart.** The service pod holds the RWOP volume, so a
   second pod cannot mount it to delete the filler. Either:
   - **expand the PVC online**, the production recovery path of this section
     (`kubectl -n airs-mcp-qual patch pvc <data-pvc> -p '{"spec":{"resources":{"requests":{"storage":"<new>Gi"}}}}'`),
     or
   - **remove the filler from inside the running pod** with an ephemeral debug container
     that shares the pod's volumes. The service image has no shell, so the container is
     the pinned busybox, as UID 65532, with the `data` volume mounted
     (`kubectl debug -n airs-mcp-qual <pod> --custom=qual-debug.json`, where
     `qual-debug.json` is an `EphemeralContainer` with `image`,
     `securityContext.runAsUser: 65532` and `volumeMounts: [{name: data, mountPath: /data}]`),
     then `rm /data/qual-filler/fill` in it. The ephemeral container goes with the pod.

   Then confirm:
   - both `airs_mcp_storage_low_space` series return to 0;
   - the blocked report is requeued and completes (`jobs get` shows `succeeded`, and
     `airs runtime history download --mcp qual --version-id <v1_…>` verifies its files);
   - `ready`, and `airs_mcp_control_ok 1`.
8. **Clean up**, in this order:
   1. `airs mcp logout qual` if a session is saved, then `airs mcp connections remove qual`;
   2. `airs gateway mcp integrations delete <qual-integration-uuid>`;
   3. the Entra registrations: `az ad app delete --id <gw-qual-client-id>` and
      `az ad app delete --id <qual-api-app-id>` (their service principals go with them);
   4. the CoreDNS rewrite and the `TRUSTED_CUSTOM_HOSTS` entry for the qualification
      hostname (talos-cluster, merged through Argo);
   5. `kubectl delete namespace airs-mcp-qual` (the Certificate, its Secret, the PVC and
      the filler directory go with it; there was no public DNS record to remove).

   Then confirm with §7a item 2 against the **production** API that only the production
   gateway client remains consented, and with item 4 that no integration targets the
   qualification URL.

**Measured capacity.** Account for each consumer separately; they grow differently.
- For the first 30 days, record weekly:
  - `airs_mcp_archive_bytes` and `airs_mcp_control_bytes` (the live databases, `D`);
  - `airs_mcp_data_volume_size_bytes` and the used bytes (size minus free);
  - the size of `DATA/backups/` (`du -sb` from a UID 65532 debug pod), and the number of
    sets in it.
- **Artifacts** (published report bundles, `A`) = used bytes − `D` − the backups
  directory. Never count backup sets as artifacts.
- **Retained backup sets.** `control-backup-set --keep 3` keeps the newest 3 verified
  sets. Each set is a full copy of control plus every archive (≈ `D`), plus its manifest.
  Retained = 3 × `D`.
- **Creation-time peak.** A new set is written in full and verified *before* pruning, so
  for the length of the 03:00 job there are 4 sets on the volume: peak backups = 4 × `D`.
  The job runs with the service stopped, but the space must already be free.
- Derive bytes per scan from the archive growth and the scans collected, which
  `history_status` reports.
- **Re-derive the PVC size** (§9.5), at the planning archive size:
  `D` (live) + 2 × `D` + 2 × 64 MiB + reservations + 64 MiB (the maintenance reserve) +
  4 × `D` (retained sets at their creation peak) + `A` for one year + 20%. That is about
  7 × `D` + `A` + 20%.
- **Reserve implications.** The disk reserve (`2 × D + 128 MiB + reservations + 64 MiB`)
  does not include the backup sets, so the sets must fit *outside* it. If the free space
  after a backup night is less than the reserve plus 512 MiB, publication pauses until
  the PVC is expanded. Plan the volume so that 4 × `D` plus the reserve plus 512 MiB stays
  below the 85% alert. Record whether 100 GiB still holds, and the measured `D`, `A` and
  per-set size.

## Rollback

- **Bad release:** set the previous image digest and re-apply. `Recreate` stops the old pod
  first. The data subtree is never repaired or recursed by the init container.
- **Corrupt store:** do not delete it. For the archive, use the guarded recovery
  online: `airs runtime history recover --mcp <name>`, the `lifecycle_preview` and
  `lifecycle_execute` action `recover`. For the control store, scale to 0 and follow
  step 11.3 with a maintenance pod. The `control-*` commands refuse while the service
  runs, including in maintenance mode, which answers only `service_info`.
