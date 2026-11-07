#!/usr/bin/env bash
# M8.1 backup-policy check (contract §9.4, plan revision 7 amendment 4): no
# agent-accessible credential can delete objects in the backup bucket. USER-RUN.
#
# Read-only. It never deletes, writes or lists bucket objects, never prints or stores a
# credential value, and makes only these Cloudflare API reads for a token you paste at
# a hidden prompt: GET /user/tokens/verify and GET /user/tokens/<id>.
#
#   scripts/m8/check-r2-delete-rights.sh [evidence-file]
#
# Part 1 lists the NAMES (never values) of credential-like environment variables, AWS
# profiles and rclone remotes visible in this workspace, so you know what an agent here
# could use. Part 2 inspects, for each token you choose to paste, the permission groups
# it holds and flags any that can write or remove R2 objects. An R2 S3 access key
# inherits its parent API token's permissions: inspect the parent token.
set -Eeuo pipefail
umask 077
BUCKET="${R2_BACKUP_BUCKET:-truffles-prod-backups}"
EVIDENCE="${1:-.local/m8-evidence/r2-delete-rights-$(date -u +%Y%m%dT%H%M%SZ).md}"
mkdir -p "$(dirname "$EVIDENCE")"
log() { printf '%s\n' "$*" | tee -a "$EVIDENCE"; }
log "# R2 delete-rights check ($(date -u +%FT%TZ)); bucket: $BUCKET"
log ""
log "## Part 1: credential names visible to this workspace (values never read)"
env | cut -d= -f1 | grep -E '^(CLOUDFLARE|CF_|R2_|AWS_ACCESS_KEY_ID|AWS_SECRET_ACCESS_KEY|AWS_SESSION_TOKEN|RCLONE_)' | sed 's/^/- env: /' | tee -a "$EVIDENCE" || log "- env: none"
if [ -f "$HOME/.aws/credentials" ]; then
  grep -E '^\[' "$HOME/.aws/credentials" | sed 's/^/- aws profile: /' | tee -a "$EVIDENCE"
else
  log "- aws profiles: none"
fi
rc="${RCLONE_CONFIG:-$HOME/.config/rclone/rclone.conf}"
if [ -f "$rc" ]; then
  awk '/^\[/{name=$0} /^provider *= *Cloudflare/{print "- rclone remote (Cloudflare): " name}' "$rc" | tee -a "$EVIDENCE"
else
  log "- rclone remotes: none"
fi
[ -d "$HOME/.wrangler" ] || [ -f "$HOME/.config/.wrangler/config/default.toml" ] && log "- wrangler login state present" || log "- wrangler login state: none"
log ""
log "## Part 2: token permission inspection"
risky='R2 Storage Write|R2 Storage Bucket Item Write|Workers R2 Storage Write|Account Settings Write|Administrator|Super Administrator'
while :; do
  read -r -p "Inspect a Cloudflare API token? [y/N] " yn
  [[ "$yn" =~ ^[Yy]$ ]] || break
  read -r -p "Label for this token (no secret): " label
  read -r -s -p "Paste the token (hidden; not stored): " tok; echo
  verify="$(curl -fsS -H "Authorization: Bearer $tok" https://api.cloudflare.com/client/v4/user/tokens/verify || true)"
  id="$(printf '%s' "$verify" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d.get("result",{}).get("id",""))' 2>/dev/null || true)"
  status="$(printf '%s' "$verify" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d.get("result",{}).get("status","unknown"))' 2>/dev/null || echo unknown)"
  if [ -z "$id" ]; then
    log "- $label: verify failed (token invalid, or not a user API token: inspect it in the dashboard)"
    unset tok; continue
  fi
  detail="$(curl -fsS -H "Authorization: Bearer $tok" "https://api.cloudflare.com/client/v4/user/tokens/$id" || true)"
  unset tok
  groups="$(printf '%s' "$detail" | python3 -c 'import json,sys
d=json.load(sys.stdin).get("result") or {}
for p in d.get("policies",[]):
    for g in p.get("permission_groups",[]):
        print(p.get("effect","?")+": "+g.get("name","?"))' 2>/dev/null || true)"
  if [ -z "$groups" ]; then
    log "- $label (status $status): cannot read its own policies; inspect it in the dashboard"
  elif printf '%s\n' "$groups" | grep -E "^allow: .*($risky)" >/dev/null; then
    log "- $label (status $status): **CAN WRITE OR REMOVE R2 OBJECTS**: narrow or revoke before deployment:"
    printf '%s\n' "$groups" | sed 's/^/    - /' | tee -a "$EVIDENCE"
  else
    log "- $label (status $status): no R2 write permission group found"
    printf '%s\n' "$groups" | sed 's/^/    - /' | tee -a "$EVIDENCE"
  fi
done
log ""
log "Verdict (fill in): every agent-accessible credential listed in Part 1 was inspected and none can remove objects in $BUCKET: yes / no"
echo "Evidence written to $EVIDENCE (mode 0600)."
