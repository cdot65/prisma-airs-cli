# M0.6 live feasibility spike — runbook and receipt template

Status: **executed 2026-11-05** (see the execution log). Step 10.3 was not run, and
M0 is not accepted until the combined record passes independent review. M0.6 is the
go/no-go gate before M1 ([contract §12](RUNTIME_MCP_CONTRACT.md)). The original text
assigned resource changes and sign-ins to the user; in execution the agent provisioned
and probed with the owner's authorization, and the owner did the console and browser
steps. Loopback
results ([feasibility record](RUNTIME_MCP_M0_FEASIBILITY.md)) cannot pass this gate.

The spike is disposable. It holds **no AIRS credentials and no customer history**.
Its only data is a synthetic SQLite drill database with a synthetic marker string.
Artifacts: `prototypes/runtime-mcp/deploy/m0.6/` (image, manifests, templates), the
server `airs-mcp-spike`, and the CLI prototype `airs-mcp-spike-cli`.

## Receipt rules (read first)

- Never paste tokens, authorization codes, client secrets, refresh tokens, cookies,
  passphrases or full `kubectl get secret` output. The spike logs contain claim
  shapes (claim names → JSON types), fixed codes, and the bounded public identifiers
  listed in contract §2.2 ("Spike diagnostics"). Copy those JSON lines
  verbatim.
- Identifiers you may record: workspace/organisation IDs, client IDs, object IDs and
  the tenant ID, as they appear in public configuration. Record them only if you are
  comfortable; otherwise write `present`/`matches config`.
- For each step, record the date/time (UTC), the command (with secrets elided), the
  exit code/HTTP status, and the copied sanitized output. Put receipts in
  `.local/dev/runtime-mcp/m0.6/` and summarize them in the receipt template at the end.

## Execution log (2026-11-05)

Steps 1.1–1.3, 2, 3, 4 and the integration provisioning were executed by the agent
with the user's authorization. Sanitized receipts: `.local/dev/runtime-mcp/m0.6/`.
Deviations from the original text: own namespace `airs-mcp`; Entra provisioning by
`deploy/m0.6/entra/provision.sh` (Admin scope not consented); route binding
(contract §2.2; later found unusable: the gateway does not forward custom headers on
`oauth_auto`, so proposed resolution 3 became the upstream-token binding); fixed loopback ports for the Idira clients
(`airs-cli` → `http://127.0.0.1:53682/callback`, `airs-admin-cli` →
`http://127.0.0.1:53683/callback`; pass `--port`); image built with rootless podman and
pushed to the private Harbor project `airs-mcp`. The deployed manifests live in
`talos-cluster/airs-mcp/`.

Outcome: steps 5–9 and 11 were run (user actions: Idira clients, the workspace MCP
server and AIS access policy, browser sign-ins, Claude Code calls). Step 7.6 confirmed
that the gateway multiplexes agents of one user onto one backend session; contract
§2.3 now makes M1 stateless, and the stateless rerun passed live (concurrent, reconnect, restart). Step 10.3 was not run, by owner decision. The P1–P9
outcome is contract §12.1. Escrow: the owner holds the offline copy.

## Step 0 — Authorization and versions

Confirm you authorize these disposable resources in namespace `airs-mcp` (own namespace:
`goshawks` enforces the restricted Pod Security level, which forbids the CHOWN init
container) and
cluster scope: one Deployment, Service, Certificate, NetworkPolicies, an RWOP PVC,
an encrypted StorageClass, an `airs-m06-restore` namespace for the restore drill,
Entra/Idira app registrations, a gateway integration, a CoreDNS rewrite and a Conjur
variable. Record the versions: gateway image/chart, Claude Code (`claude --version`),
Talos/Kubernetes, Longhorn, cert-manager, and the spike image digest.

## Step 1 — Identity registrations (new IDs; never reuse Bonbon's)

1. **Entra API app `airs-mcp-api`.** Identifier URIs `api://<appId>` **and**
   `https://mcp-backend-airs.goshawks.io/mcp`. Delegated scopes `AirsHistory.Read`,
   `AirsHistory.Operate`, `AirsHistory.Admin` (exposed under the MCP URI). Record
   `appId` (→ `upstream.audiences`) and the tenant ID.
2. **Gateway confidential client** for `oauth_auto`, as Bonbon's integration uses it
   (`client_secret_post`). Grant it the delegated scopes above. Record its client ID
   (→ `upstream.allowed_client_ids`). Keep the secret in the gateway provisioning
   flow only.
3. **Integration owner consent.** The one-time owner consent creates the upstream
   delegation. Record the owner `oid` (→ `owner_object_ids` and each grant's
   `owner_object_id`).
4. **CLI public clients** at the caller authorization server the gateway advertises
   (Idira): `airs-cli` (ordinary) and `airs-admin-cli` (admin). Public, PKCE S256,
   loopback redirect `http://127.0.0.1/callback` (any port, RFC 8252), refresh tokens
   enabled for `airs-cli` only. Record both client IDs.
5. Fill every `PLACEHOLDER_*` in `deploy/m0.6/spike-config.toml`, then run
   `airs-mcp-spike check-config --config spike-config.toml` (expect `config_ok`).

## Step 2 — Encrypted storage and key custody (contract §9.4)

1. Generate a passphrase and store it as Conjur `data/airs-mcp/backup/volume-passphrase`
   (`cluster/conjur-policy.snippet.yml`). **Escrow an offline copy.** Record only
   "escrowed: yes/no" and its location class (e.g. "password manager", "safe").
2. Add a `longhorn-system` SecretStore following the cluster's ESO pattern, then apply
   `cluster/longhorn-crypto-externalsecret.yaml` and `cluster/storageclass-encrypted.yaml`.
3. Record your backup policy decisions: target (existing Longhorn target nas2 → nas1
   → R2, or another), RPO (proposed 24 h), retention (proposed: keep until explicit
   prune; reconcile with the existing 14 daily/8 weekly RecurringJobs), restore-drill
   cadence (proposed monthly) and drill owner.

## Step 3 — Image

```sh
DOCKER_BUILDKIT=1 docker build --platform linux/amd64 \
  -f prototypes/runtime-mcp/deploy/m0.6/Dockerfile -t registry.cdot.io/airs-mcp/spike:m0.6 .
docker push registry.cdot.io/airs-mcp/spike:m0.6
docker buildx imagetools inspect registry.cdot.io/airs-mcp/spike:m0.6 --format '{{.Manifest.Digest}}'
```

Record the pushed digest and the in-image `/usr/local/share/airs-mcp-spike.sha256`.
Pin the digest in `kustomization.yaml`, `contender-pod.yaml` and
`restore/verify-job.yaml`. Pin a busybox digest for the init container.

## Step 4 — Deploy and route

1. Copy `deploy/m0.6/` into talos-cluster, following its AGENTS.md (for example
   `goshawks/airs-mcp-spike/`). Apply through Argo CD or `kubectl apply -k`.
2. Add `cluster/coredns-rewrite.snippet` beside the Bonbon block, and add
   `mcp-backend-airs.goshawks.io` to the gateway's `TRUSTED_CUSTOM_HOSTS`.
3. Provision the gateway integration from `gateway/integration.template.json` with
   the repository's `airs gateway mcp integrations` command, as Bonbon's
   `provision.py` does. Enable identity forwarding. **Record whether the gateway
   offers a per-integration `aud`/binding claim or a per-integration signing key**
   (contract §2.2), with the setting names, for P1.
4. Receipts: the `data-subtree` init container log (expect `0:0 755` for `/data` and
   `65532:65532 700` for `/data/airs`). Spike log lines `storage_owned` (expect
   `"filesystem":"ext4"`) and `listening` (`"tls":true`).
   `curl -s https://mcp-backend-airs.goshawks.io/.well-known/oauth-protected-resource/mcp`
   from an allowed path is the metadata receipt.

## Step 5 — First real call and claim shapes (P1, P2)

1. In Claude Code: `claude mcp add --transport http airs https://mcp.goshawks.io/<workspace>/airs/mcp`,
   sign in, then ask it to call `service_info`.
2. Copy the spike's `allowed` or `denied` JSON log line. It contains
   `upstream_shape` and `assertion_shape`.
   - `denied` with reason `identity assertion is not bound to this backend` means the
     gateway does not sign the per-backend binding. **P1 fails as configured.** Record
     the shape and the gateway settings available, then stop the positive-path steps
     and resolve P1 (contract §2.2) before continuing. The spike has no setting that
     accepts an unbound assertion. (M0.6 outcome: this denial occurred; P1 is resolved
     by resolution 3, plan revision 7 amendment 1.)
   - `denied` for `iss`: set `assertion.issuer` to the gateway's documented issuer.
     The shape shows `iss` presence, never its value. Do not guess from a token.
   - `allowed`: record both shapes, the `permission`, and the `negotiated_protocol_header`
     from the `service_info` result.

## Step 6 — Positive path and response bounds (P6)

Through Claude Code, call:

- `echo` with `{"text":"xxxx","repeat":N}` for N = 1024, 4096, 16384 and 30000
  (4, 16, 64 and 120 KB of text). The complete result is about twice the text, so
  30000 is about 240 KB, just inside the 256 KiB result bound. As a control,
  N = 50000 must be refused **locally by the spike** with `result exceeds its
  response bound`; that is not a gateway measurement;
- `text_page` until `next_cursor` is null;
- `artifact_chunk` with `{"offset":0,"length":65536}`.

Record, for each call, whether Claude Code received it intact (it reports `bytes`
and `sha256`) and any truncation or error. Expected sizes: each `text_page` result is
≤16 KiB complete, and an `artifact_chunk` of 64 KiB is about 175 KiB complete,
because rmcp sends the structured value twice (contract §5). Note whether Claude Code warned about
output size, and the `MAX_MCP_OUTPUT_TOKENS` setting if you changed it.

## Step 7 — Negative identity cases through the real path

Each must be **denied**. Record the spike `denied` line or the HTTP status.

1. Unknown agent: call from a second MCP client/agent registration that has no grant.
2. Unknown caller: a second user who passes gateway policy but has no grant.
3. Wrong backend binding: temporarily set `assertion.audiences` to
   `["https://mcp-backend-other.goshawks.io/mcp"]`, sync, call (expect `forbidden`,
   not bound), then revert.
4. Wrong workspace: temporarily replace `workspace_ids`, then revert.
5. Reader → Operator: from the Claude Code (Reader) grant, call `operator_noop`
   (expect a tool result with `code: forbidden`).
6. Session isolation: with two different granted callers or agents connected through
   the gateway at once, both must keep working. Record any `denied` line with reason
   `session belongs to another principal`. That line means the gateway multiplexes
   callers onto one backend MCP session, which needs a qualified isolation mechanism
   before M1. (M0.6 result: confirmed, 15/15 Reader requests refused while the admin
   CLI was active. Rerun on the stateless transport, revision `m06-7`: Claude Code 15/15
   ok while the admin CLI looped, with 0 denials and no `Mcp-Session-Id` reaching the
   backend. After a pod restart, Claude Code recovered without reconnecting: one
   gateway 502 that never reached the new pod, then ok. Receipts rows "7.6 stateless …".)
7. Bypass attempt: from a pod in `goshawks` without the gateway labels, connect to
   `airs-mcp.airs-mcp.svc:443` (expect a NetworkPolicy timeout). The spike has no
   unauthenticated MCP route, so a request with no tokens gets 401.

## Step 8 — CLI login, refresh and interruption (P5)

Build `airs-mcp-spike-cli` (`cargo build --release -p airs-mcp-spike-cli --manifest-path prototypes/runtime-mcp/Cargo.toml`). With `URL=https://mcp.goshawks.io/<workspace>/airs/mcp`:

```sh
# Approve the gateway origin AND the authorization server origin(s) it advertises.
A="--approved-origin https://mcp.goshawks.io --approved-origin <idira-origin>"
airs-mcp-spike-cli call   --url "$URL" $A --tool service_info      # expect exit 1: authentication required
airs-mcp-spike-cli login  --url "$URL" $A --client-id <airs-cli> --port 53682   # only command that starts sign-in
airs-mcp-spike-cli status --url "$URL" $A                          # booleans only, never token values
airs-mcp-spike-cli call   --url "$URL" $A --tool service_info
airs-mcp-spike-cli refresh --url "$URL" $A                         # records refresh_token_rotated
airs-mcp-spike-cli download --url "$URL" $A --out /tmp/m06-artifact.bin   # records calls/elapsed/max_result_bytes
airs-mcp-spike-cli login  --url "$URL" $A --client-id <airs-cli> --port 53682   # press Ctrl-C before finishing; then:
airs-mcp-spike-cli status --url "$URL" $A                          # previous session is unchanged
airs-mcp-spike-cli login  --url "$URL" --approved-origin https://mcp.goshawks.io --client-id <airs-cli> --port 53682
                                                                   # expect refusal: AS origin not approved
airs-mcp-spike-cli logout --url "$URL" $A; airs-mcp-spike-cli call --url "$URL" $A --tool service_info  # exit 1
```

Record exit codes and the JSON lines. Run `ls -la ~/.local/state/airs-cli-spike/sessions`
and record the modes only (expect `drwx------` and `-rw-------`).

## Step 9 — Administrative authority (P3)

1. With the `airs-admin-cli` grant, call `admin_preview {"parameter":"m06"}`, then
   `admin_execute` with the returned `preview_id`. Expect denial: `admin.mode` is
   `disabled`. Record the line.
2. Shell-context probe: from the Claude Code agent's own Bash tool, using a cached
   `airs-admin-cli` session, attempt `admin_execute` **and** the lower-tier mutation
   `operator_noop`. Both must be denied: an administrative identity mutates only
   through proof-bound tools (contract §3.4).
3. Option B is qualified only if the gateway can sign a forwarded assertion carrying
   a server nonce and `auth_time` for one operation. Record whether any such gateway
   feature exists. If none exists, record "admin mutation disabled". That is
   acceptable, and the contract keeps it disabled.

## Step 10 — Storage ownership and node loss (P7)

1. With the spike running, apply `contender-pod.yaml`. Expect it to stay `Pending`.
   Record `kubectl describe pod airs-mcp-spike-contender` events (RWOP conflict).
   Then delete it.
2. Rollout overlap: `kubectl rollout restart deploy/airs-mcp-spike`. Record that the
   old pod terminated before the new one started (`Recreate`) and that the new pod
   logged a fresh `storage_owned` owner ID.
3. **Not run in M0.6 (owner decision, 2026-11-05).** Every node carries production
   workloads (talos3, hosting the spike, had 61 pods), and the owner judged a
   node-loss test unjustified. Never power off, isolate or drain a shared cluster node
   for this step. It may run only on a dedicated, disposable node the owner provides.
   Original procedure, for that case: node loss:
   power off or isolate the node hosting the spike. Record the old pod's state, the
   replacement's state, the Longhorn volume attachment state and the times. Do **not**
   force-detach while the old writer's state is uncertain. When the node returns (or
   after the documented manual Longhorn procedure you choose), record when the
   replacement acquired ownership. Any outcome where two pods mounted the volume
   reopens the design.

## Step 11 — Encrypted backup and isolated restore (P8)

```sh
M="AIRS-M06-DRILL-$(date -u +%Y%m%dT%H%M%SZ)"     # synthetic marker; not a secret
kubectl -n airs-mcp exec deploy/airs-mcp-spike -- /usr/local/bin/airs-mcp-spike drill seed   --data-dir /data/airs --rows 1000 --marker "$M"
kubectl -n airs-mcp exec deploy/airs-mcp-spike -- /usr/local/bin/airs-mcp-spike drill backup --data-dir /data/airs --marker "$M"
```

1. Record the `drill_backup` line, which names the set.
2. **Positive encryption evidence.** Record each of these:
   - `kubectl -n longhorn-system get volumes.longhorn.io <volume> -o jsonpath='{.spec.encrypted}'`,
     expecting `true`;
   - the PV's `spec.csi.volumeAttributes.encrypted`;
   - on the attached node (Talos: `talosctl -n <node> ls /dev/mapper` or
     `talosctl read /proc/mounts`), the `dm-crypt` mapping for the volume.

   Record names only, never the passphrase.
3. Create a Longhorn backup of the `airs-mcp-spike-data` volume (UI, or by triggering
   the backup RecurringJob). Record the backup URL.
4. **Wrong-key restore must fail.** Create a throwaway Secret
   `airs-mcp-volume-crypto-wrong` in `longhorn-system` with a random passphrase
   (same keys as `cluster/longhorn-crypto-externalsecret.yaml`). Set `fromBackup` in
   `restore/storageclass-wrong-key.yaml` and apply it. Then apply the restore PVC and
   verify Job with `storageClassName: longhorn-airs-restore-wrong-key`. Expect the
   volume to fail to stage or mount, and the Job never to log `drill_verified`.
   Record the events, then delete those resources and the wrong-key Secret.
5. **Correct-key restore must succeed.** Do this with the escrowed key: confirm that
   the Conjur value matches the escrow copy without printing either. Set `fromBackup`
   in `restore/storageclass-from-backup.yaml`, and the set name and marker in
   `restore/verify-job.yaml`, then `kubectl apply -k restore/`. The Job verifies the
   manifest length/hash, `PRAGMA integrity_check`, and that **every restored row
   equals `<marker>:<id>` for ids 0..999**. Record the `drill_verified` line (rows 1000,
   sha256) and the elapsed time.
6. *Supplementary only:* run `grep -rac "$M" /volume1/longhorn-backups/backupstore/volumes/<volume-dir>/`
   on the target and record the count. Zero matches is consistent with encryption,
   but it is **not proof**: compression alone can hide the marker. Steps 2, 4 and 5
   are the evidence.
7. Tear down the restore namespace, both restore StorageClasses and their volumes.

## Step 12 — Teardown

Record a keep/delete decision for each item. (M0.6 decisions, 2026-11-05: receipts row "step 12 teardown decisions"; deletions wait for review 9.)

1. The spike kustomization and the contender pod: delete.
2. The retained PV, Longhorn volume and drill backups: they hold only synthetic data;
   delete them explicitly.
3. The gateway integration and the CoreDNS/trusted-host entries: remove them unless
   M1 proceeds immediately.
4. The `longhorn-airs-encrypted` StorageClass, the `airs-mcp-volume-crypto`
   ExternalSecret/Secret and the `longhorn-system` SecretStore: keep them only if the
   encrypted class carries forward to M8. Never delete the passphrase while any
   encrypted volume or backup that needs it still exists.
5. The Conjur variable and its offline escrow: delete them only after every backup
   of every volume encrypted with that passphrase has been pruned.
6. The Entra/Idira registrations: keep or delete as you decide.

## Receipt template and go/no-go

| Gate | Evidence to attach | Pass condition |
| --- | --- | --- |
| P1 backend binding | Step 4.3 settings + step 5 shape + step 7.3 denial | A signed `aud`/binding or a per-integration key is enforced, or (plan revision 7, amendment 1) the upstream token's API `aud` plus its dedicated confidential `azp`, with the amendment's conditions; a wrong binding is denied |
| P2 upstream token | Step 5 `upstream_shape` + step 7 denials | `aud`=API app ID, delegated `scp`, `azp`, `oid`, `tid` as specified; mismatches denied |
| P3 admin authority | Step 9 | Option A or B qualified, **or** admin recorded as disabled |
| P4 grants | Steps 5–7 | Unknown agent/caller denied; Reader cannot mutate |
| P5 CLI OAuth | Step 8 | Login, refresh, noninteractive denial, interruption without corruption, unapproved-origin refusal, logout |
| P6 bounds/protocol | Steps 5–6, 8 download | Negotiated version recorded; ≤16 KiB complete pages and a ~175 KiB complete chunk delivered, or a smaller recorded fallback |
| P7 storage | Steps 4, 10 | Fixed UID 0700 subtree on ext4; contender Pending; no overlap; fresh owner ID. Node-loss behavior with no dual mount moved to M8.2 on a dedicated disposable node (plan revision 7, amendment 3) |
| P8 backup | Steps 2, 11 | Custody recorded; encryption positively shown; wrong-key restore fails; correct-key restore verifies every row |
| P9 architecture | Step 0 | linux/amd64 nodes |

M0 passes only when P1, P2 and P4–P9 pass and P3 is either qualified or recorded
as disabled. Then an independent review accepts the combined M0 record at ≥9.1.
Any failure is recorded in `docs/RUNTIME_MCP_PHASE_REVIEWS.md` together with the
design question it reopens.
