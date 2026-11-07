#!/usr/bin/env bash
# M8.3 user-run checklist: walks the steps only the owner can perform and records the
# evidence into a local file (0600). It never prints, stores or asks for a secret: free
# text that looks like a credential (long high-entropy tokens, PEM blocks, JWTs) is
# refused. It runs nothing against the cluster or any service itself; each step shows
# the command for you to run and asks for the result. docs/RUNTIME_MCP_DEPLOYMENT_RUNBOOK.md
# is the full procedure.
#
#   scripts/m8/user-checklist.sh [evidence-file]
set -Eeuo pipefail
umask 077
EVIDENCE="${1:-.local/m8-evidence/m8-user-evidence-$(date -u +%Y%m%dT%H%M%SZ).md}"
mkdir -p "$(dirname "$EVIDENCE")"
: > "$EVIDENCE"; chmod 600 "$EVIDENCE"
rec() { printf '%s\n' "$*" >> "$EVIDENCE"; }

looks_secret() {
  local s="$1"
  [[ "$s" == *"-----BEGIN"* ]] && return 0
  [[ "$s" =~ eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\. ]] && return 0
  [[ "$s" =~ [A-Za-z0-9+/_=-]{40,} ]] && return 0
  return 1
}

ask() {
  local prompt="$1" answer
  while :; do
    read -r -p "$prompt " answer
    if looks_secret "$answer"; then
      echo "  That looks like a credential; record a description or a file path instead." >&2
      continue
    fi
    printf '%s' "$answer"; return
  done
}

step() {
  local n="$1" title="$2" how="$3"
  echo; echo "== Step $n: $title"; echo "$how"
  rec ""; rec "## Step $n: $title"
  local done result evidence
  done="$(ask "Done? [yes/no/skip]")"
  result="$(ask "Result in one line (no secrets):")"
  evidence="$(ask "Evidence file path(s), if any:")"
  rec "- status: $done"; rec "- result: $result"
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
issuer/audiences, grants, scanner revision). Then: cargo test -p airs-mcp --test deploy_manifests"
step 2 "Build, push and pin the image" \
"docker build --platform linux/amd64 -f deploy/airs-mcp/Dockerfile -t <registry>/airs-mcp:<version> .
Then docker push <registry>/airs-mcp:<version> and put the pushed digest in deploy/airs-mcp/kustomization.yaml."
step 3 "Encrypted storage class and secrets" \
"Confirm the longhorn-airs-encrypted StorageClass exists with its Conjur/ESO passphrase, and the
ESO secrets airs-mcp-airs-credentials and airs-mcp-tls (cert-manager) are Ready."
step 4 "nas1 retention record (contract §9.4)" \
"On nas1 (DSM, read-only inspection), record whether the nas2 -> nas1 sync mirrors deletions and
its retention/rotation. Record the setting names and a screenshot path."
step 5 "No agent-accessible R2 delete right (contract §9.4)" \
"Run scripts/m8/check-r2-delete-rights.sh from each agent workspace image (Coder) and record the
evidence file it writes. Narrow any credential it flags before continuing."
step 6 "Deploy" \
"kubectl apply -k deploy/airs-mcp (your GitOps path). Confirm one pod Running, the PVC Bound
(ReadWriteOncePod), /readyz 200 and /metrics serving from the pod (kubectl port-forward 8081)."
step 7 "Gateway sign-in and service proof" \
"airs mcp login <connection>; airs mcp status <connection>; airs runtime history status --mcp <connection>.
Record the outputs (they contain no secrets)."
step 8 "Claude Code proof" \
"Connect Claude Code to the gateway MCP URL; call service_info and history_status. Record the result."
step 9 "Backup marker test (runbook)" \
"Write the runbook's plaintext marker, take a Longhorn backup, and confirm the marker is NOT found
on the backup target (ciphertext). Record the search command and its result."
step 10 "Node-loss test on a DEDICATED, DISPOSABLE node only (contract §9.3)" \
"Never a shared talos node. Follow the runbook's node-loss procedure on a node you can lose, and
record whether the volume moved without force-detach and the service recovered."
step 11 "Restore drill (contract §9.4)" \
"Restore the latest Longhorn backup into an isolated namespace, start airs-mcp in maintenance mode,
and record history_status and the archive integrity result."

rec ""; rec "End of evidence."
echo; echo "Evidence written to $EVIDENCE (mode 0600). Attach it to the M8 review packet."
