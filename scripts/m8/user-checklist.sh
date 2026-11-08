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
step 4 "Off-volume backup policy (operator record; optional)" \
"If your environment copies the data volume or the backup sets elsewhere, record where, the retention
and rotation, who can delete them, and whether a mirror propagates deletions. Not an acceptance gate
(contract §9.4, 2026-11-07): answer skip with the reason 'no off-volume copies' when none exist."
step 5 "Agent access to backup copies (operator record; optional)" \
"If agent workspaces can reach credentials for an off-volume backup store, confirm none can delete backup
objects. For a Cloudflare R2 bucket run scripts/m8/check-r2-delete-rights.sh on a sanitized inventory and
record its evidence file (exit 0). Optional: answer skip with a reason when not applicable."
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
# Step 10 (the node-loss test) was withdrawn by the owner on 2026-11-08, permanently (plan
# revision 7 amendment 3). It is recorded, not asked; steps 11 and 12 keep their numbers.
echo; echo "== Step 10: Node-loss test (withdrawn)"
echo "Withdrawn by the owner on 2026-11-08; nothing to do. Runbook §10 is operator recovery only."
rec ""; rec "## Step 10: Node-loss test (withdrawn)"
rec "- status: withdrawn"
rec "- result: not an acceptance step (owner decision 2026-11-08, permanent; plan revision 7 amendment 3)"
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
step 12 "Capacity evidence (runbook §12, contract §9.5)" \
"By owner decision (2026-11-08) the capacity evidence is the planning-size measurement, not a
30-day series: 5,000,000 synthetic scans, archive D, verified set size, three retained sets and
the fourth set's creation peak, all under the 2 GiB limit, and the runbook formula (about 7 x D +
one year of artifacts + 20%). Record the measurement file and whether 100 GiB holds. The low-space
exercise was withdrawn by the owner on 2026-11-08."

rec ""; rec "End of evidence."
echo; echo "Evidence written to $EVIDENCE (mode 0600). Attach it to the M8 review packet."
