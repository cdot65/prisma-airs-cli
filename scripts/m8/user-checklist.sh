#!/usr/bin/env bash
# M8.3 user-run checklist: walks the steps only the owner can perform and records the
# evidence into a local file (0600). It never prints, stores or asks for a secret, and it
# runs nothing against the cluster or any service itself: each step shows the command
# for you to run and asks for the result. docs/RUNTIME_MCP_DEPLOYMENT_RUNBOOK.md is the
# full procedure.
#
#   scripts/m8/user-checklist.sh [evidence-file]
#   scripts/m8/user-checklist.sh --check KIND    (reads one value on stdin; for tests)
#
# Every answer is validated by its field kind (scripts/m8/lib-secrets.sh):
#   - status: exactly pass, fail or skip (skip requires a reason as the result);
#   - hash: the image digest, exactly sha256:<64 lowercase hex>;
#   - text: free text, refused when it looks like a credential (PEM, JWT, Authorization
#     header, password=/secret=/token=/api_key=… pairs, AWS keys, long high-entropy runs).
# A refused answer is never echoed or recorded; you are asked again.
#
# --check exit codes: 0 accepted, 3 refused (looks like a credential), 4 invalid for the
# kind, 2 usage.
set -Eeuo pipefail
umask 077
# shellcheck source=scripts/m8/lib-secrets.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-secrets.sh"

if [ "${1:-}" = "--check" ]; then
  kind="${2:-}"
  [ -n "$kind" ] || { echo "usage: user-checklist.sh --check text|hash|status" >&2; exit 2; }
  IFS= read -r value || true
  rc=0; m8_check "$kind" "$value" || rc=$?
  exit "$rc"
fi

EVIDENCE="${1:-.local/m8-evidence/m8-user-evidence-$(date -u +%Y%m%dT%H%M%SZ).md}"
mkdir -p "$(dirname "$EVIDENCE")"
: > "$EVIDENCE"; chmod 600 "$EVIDENCE"
rec() { printf '%s\n' "$*" >> "$EVIDENCE"; }

# ask KIND PROMPT [optional]: an accepted answer of KIND (empty allowed when optional).
ask() {
  local kind="$1" prompt="$2" optional="${3:-}" answer rc
  while :; do
    read -r -p "$prompt " answer
    if [ -z "$answer" ] && [ -n "$optional" ]; then printf ''; return; fi
    rc=0; m8_check "$kind" "$answer" || rc=$?
    case "$rc" in
      0) printf '%s' "$answer"; return ;;
      3) echo "  That looks like a credential; it was not recorded. Describe it, or give a file path." >&2 ;;
      *) case "$kind" in
           status) echo "  Answer pass, fail or skip." >&2 ;;
           hash) echo "  Give the digest as sha256:<64 lowercase hex>." >&2 ;;
           *) echo "  Not accepted; try again." >&2 ;;
         esac ;;
    esac
  done
}

step() {
  local n="$1" title="$2" how="$3" digest="${4:-}"
  echo; echo "== Step $n: $title"; echo "$how"
  rec ""; rec "## Step $n: $title"
  local status result evidence pin
  status="$(ask status "Status [pass/fail/skip]:")"
  if [ "$status" = "skip" ]; then
    result="$(ask text "Reason for skipping (required; no secrets):")"
    while [ -z "$result" ]; do result="$(ask text "Reason for skipping (required; no secrets):")"; done
  else
    result="$(ask text "Result in one line (no secrets):")"
  fi
  if [ -n "$digest" ] && [ "$status" = "pass" ]; then
    pin="$(ask hash "Pinned image digest (sha256:...):")"
    rec "- digest: $pin"
  fi
  evidence="$(ask text "Evidence file path(s), if any:" optional)"
  rec "- status: $status"; rec "- result: $result"
  if [ -n "$evidence" ]; then
    for f in $evidence; do
      if [ -f "$f" ]; then rec "- evidence: $f sha256 $(sha256sum "$f" | cut -d' ' -f1)"; else rec "- evidence (not found here): $f"; fi
    done
  fi
}

rec "# M8 user-run evidence ($(date -u +%FT%TZ))"
rec "commit: $(git rev-parse HEAD 2>/dev/null || echo unknown)"

step 1 "Fill the production configuration" \
"Replace every placeholder in deploy/airs-mcp/airs-mcp.toml (tenant, Entra IDs, assertion
issuer/audiences, grants, scanner revision); scripts/m8/derive-config.sh derives them interactively.
Then: cargo test -p airs-mcp --test deploy_manifests"
step 2 "Build, push and pin the image" \
"docker build --platform linux/amd64 -f deploy/airs-mcp/Dockerfile -t <registry>/airs-mcp:<version> .
Then docker push <registry>/airs-mcp:<version> and put the pushed digest in deploy/airs-mcp/kustomization.yaml." \
digest
step 3 "Encrypted storage class and secrets" \
"Confirm the longhorn-airs-encrypted StorageClass exists with its Conjur/ESO passphrase, and the
ESO secrets airs-mcp-airs-credentials and airs-mcp-tls (cert-manager) are Ready."
step 4 "nas1 retention record (contract §9.4)" \
"On nas1 (DSM, read-only inspection), record whether the nas2 -> nas1 sync mirrors deletions and
its retention/rotation. Record the setting names and a screenshot path."
step 5 "No agent-accessible R2 delete right (contract §9.4)" \
"From an owner workstation (not an agent workspace), export the sanitized credential inventory of each
agent workspace image as described in scripts/m8/check-r2-delete-rights.sh, then run it on that file
and record the evidence file it writes. It must exit 0; narrow any credential it flags first."
step 6 "Deploy" \
"kubectl apply -k deploy/airs-mcp (your GitOps path). Confirm one pod Running, the PVC Bound
(ReadWriteOncePod), /readyz 200 and /metrics serving from the pod (kubectl port-forward 8081)."
step 7 "Gateway sign-in and service proof" \
"airs mcp login <connection>; airs mcp status <connection>; airs runtime history status --mcp <connection>.
Record the outputs (they contain no secrets)."
step 8 "Claude Code proof" \
"Connect Claude Code to the gateway MCP URL; call service_info and history_status. Record the result."
step 9 "Coordinated backup set and marker test (runbook §9a, §9b)" \
"With the service stopped (Deployment at 0), in a maintenance pod:
  airs-mcp control-backup-set --config /etc/airs-mcp/config/airs-mcp.toml
    (writes the coordinated airs-mcp-backup/1 set to /data/airs/backups/<id>/; record the
     {\"event\":\"backup_set\",...} line and its set id)
  airs-mcp control-backup-verify --set /data/airs/backups/<id>
    (record the {\"event\":\"backup_set_verified\",...} line)
This coordinated set is the content backup. It is NOT the input of control-restore (step 11).
Then the report-title marker test: proven present in the stored data, not found on the backup
target, with positive encryption evidence, a wrong-key restore refused and a correct-key restore
showing the marker. Record the commands and results."
step 10 "Node-loss test on a DEDICATED, DISPOSABLE node only (contract §9.3)" \
"Never a shared talos node. Follow the runbook's node-loss procedure on a node you can lose, and
record whether the volume moved without force-detach and the service recovered."
step 11 "Restore drill (contract §9.4)" \
"Follow runbook §11 in the isolated drill namespace (default-deny egress, no production
credentials). Restore the volume with the drill Deployment at 0, then in a maintenance pod, in order:
  1. airs-mcp control-backup --config /etc/airs-mcp/config/airs-mcp.toml --dir /data/airs/backups/drill-<date>
     (a control-store-only set, the format control-restore reads; record the control_backup line)
  2. move control/control.sqlite3* aside into /data/airs/control-preserved-<date>/ (keep seed.json)
  3. airs-mcp control-restore --config /etc/airs-mcp/config/airs-mcp.toml --set /data/airs/backups/drill-<date>
     (record the control_restored line and its pending_epoch)
  4. airs-mcp control-activate-epoch --config /etc/airs-mcp/config/airs-mcp.toml --epoch <pending_epoch>
     (record the control_epoch_activated line)
Do not run control-reconcile in the drill. Optionally re-verify the restored content with
airs-mcp control-backup-verify --set /data/airs/backups/<id>. Then the controlled startup and the
marker check. Record each event line and the content verification."
step 12 "Low-space qualification and measured capacity (runbook §12, contract §9.5)" \
"Use the SEPARATE qualification deployment, never the restore-drill namespace (its copy stays
quarantined and has no route). Follow runbook §12 'Qualification on the actual storage class':
 1. namespace airs-mcp-qual: default-deny NetworkPolicy (ingress only gateway pods -> TLS port;
    egress only DNS, the Entra JWKS/metadata and the gateway assertion JWKS; NO AIRS endpoint);
    its own backend hostname mcp-backend-airs-qual.goshawks.io and Certificate; placeholder AIRS
    secret files; a small encrypted PVC. The qual overlay's data-subtree init container also
    creates /data/qual-filler (0700, 65532:65532) beside /data/airs; /data stays root 0755.
    The config differs from production ONLY as the §12 step-1 table says: allowed_hosts/resource/
    scopes_supported -> the qual hostname; [upstream].audiences -> the qual API app ID;
    allowed_client_ids -> the qual gateway client. Everything else unchanged.
 2. its OWN identities (§12 step 2; the production gateway client serves one integration only):
    2.1 a qualification API registration (az ad app create ... --identifier-uris
        https://mcp-backend-airs-qual.goshawks.io/mcp; copy the production scopes with new IDs);
    2.2 a confidential-only qualification gateway client with those scopes admin-consented;
        record its keyId, never the secret value;
    2.3 'airs gateway mcp integrations create --file prototypes/runtime-mcp/deploy/m0.6/gateway/
        integration.template.json --name airs-qual --slug airs-qual --transport http --url
        https://mcp-backend-airs-qual.goshawks.io/mcp --auth-type oauth_auto ...' (record the UUID);
    2.4 the CoreDNS rewrite to airs-mcp.airs-mcp-qual.svc.cluster.local and the trusted-host
        entry; no public DNS record;
    2.5 REPEAT §7a items 1-4 for the qual client, the qual API SP, its keyId and the qual URL;
        refuse and stay blocked if any item fails;
    2.6 then 'airs mcp connections set qual ...' and 'airs mcp login qual'.
 3. seed: fresh ('airs runtime receipts upload --mcp qual --from-bulk-state <synthetic checkpoint>'),
    or restore per runbook 11.2-11.3.4 and reconcile DELIBERATELY with the Deployment at 0:
    'airs-mcp control-reconcile --config /etc/airs-mcp/config/airs-mcp.toml'. Schedules stay disabled.
 4. start at 1 replica; 'airs runtime history status --mcp qual' and /readyz = ready.
 5. Deployment at 0 (RWOP: one pod at a time); from a UID 65532 busybox debug pod mounting the PVC,
    fill /data/qual-filler/fill (fallocate, or dd) until about 1% is free; stat shows
    65532:65532 700 for the directory; delete the debug pod before step 6. Never fill inside
    /data/airs (the reserve would count it as database bytes).
 6. start; record storage_owned/control_recovered (or the refusal), /readyz, the volume metrics,
    airs_mcp_storage_reserve_bytes and both airs_mcp_storage_low_space series = 1; a
    'airs runtime report --mcp qual --interval 1 --unit days --no-wait' job left blocked_policy
    ('airs runtime jobs get --mcp qual <job>'), and 'receipts upload --mcp qual' storage_unavailable.
 7. free the space WITHOUT a restart: expand the PVC online, or rm /data/qual-filler/fill from an
    ephemeral busybox debug container (UID 65532, the data volume mounted) in the running pod;
    both series back to 0, the blocked report requeued and succeeded, 'airs runtime history
    download --mcp qual --version-id <v1_...>' verifies, ready and airs_mcp_control_ok 1.
 8. clean up in order: 'airs mcp connections remove qual'; 'airs gateway mcp integrations delete
    <uuid>'; az ad app delete for the qual gateway client and the qual API; the CoreDNS rewrite
    and trusted-host entry; 'kubectl delete namespace airs-mcp-qual'. Then re-check §7a items 2
    and 4 against production: only the production client consented, nothing targets the qual URL.
Then record the 30-day capacity measurements (artifacts, retained backup sets and the creation peak
accounted separately, runbook §12 'Measured capacity') and whether 100 GiB still holds."

rec ""; rec "End of evidence."
echo; echo "Evidence written to $EVIDENCE (mode 0600). Attach it to the M8 review packet."
