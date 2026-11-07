#!/usr/bin/env bash
# M8.1 backup-policy check (contract §9.4, plan revision 7 amendment 4): no
# agent-accessible credential can delete objects in the backup bucket.
#
# OFFLINE ONLY. This helper makes no network call, reads no credential, environment
# variable or credential file, and runs no Cloudflare, AWS, rclone or cluster command.
# It validates an inventory file the owner prepares separately:
#
#   scripts/m8/check-r2-delete-rights.sh INVENTORY.json [evidence-file]
#
# What the owner does first (outside this helper, from an owner workstation, never from
# an agent workspace), for each agent workspace image (Coder template or equivalent):
#   1. List every credential that environment can reach: its environment variable NAMES,
#      AWS profile names, rclone remotes, wrangler/cf login state, mounted secret file
#      paths, and any other secret source. Record names only, never values.
#   2. For each Cloudflare API token among them, read its permission groups and resource
#      scopes in the Cloudflare dashboard (My Profile > API Tokens) or with the `cf` CLI
#      on the owner workstation. An R2 S3 access key inherits its parent API token's
#      permissions: inspect the parent token.
#   3. Write INVENTORY.json (sanitized; names, permission group names and scopes only):
#
#   {
#     "bucket": "truffles-prod-backups",
#     "environment": "coder template agent-workspace v12",
#     "inventory_complete": true,
#     "sources_checked": ["env", "aws_profiles", "rclone", "wrangler", "cf_cli",
#                         "mounted_secrets", "other"],
#     "credentials": [
#       {"name": "CF_API_TOKEN", "kind": "cloudflare_api_token", "inspected": true,
#        "permissions": ["Workers R2 Storage Read"],
#        "resources": ["account:*"]}
#     ]
#   }
#
# Strict schema (anything outside it is UNKNOWN and blocks the verdict, exit 3):
#   - top-level keys: exactly bucket, environment, inventory_complete, sources_checked,
#     credentials;
#   - credential keys: exactly name, kind, inspected, permissions, resources;
#   - kind: one of cloudflare_api_token, r2_access_key, aws_profile, rclone_remote,
#     mounted_secret, environment_secret (an R2 access key and an rclone/AWS remote hold
#     the permissions of the token or policy behind them: list those);
#   - permissions: a non-empty list of
#       * Cloudflare permission groups from this list (read-only, or modify-capable):
#           read:   Workers R2 Storage Read, Workers R2 Storage Bucket Item Read,
#                   Account Settings Read, Zone Read, DNS Read, Workers Scripts Read,
#                   Workers KV Storage Read, Account Analytics Read
#           modify: Workers R2 Storage Write, Workers R2 Storage Bucket Item Write,
#                   Account Settings Write, Zone Write, DNS Write, Workers Scripts Write,
#                   Workers KV Storage Write, Administrator, Super Administrator
#       * S3 actions `s3:<Action>` (case-sensitive): read actions s3:GetObject,
#         s3:GetObjectVersion, s3:HeadObject, s3:ListBucket, s3:ListBucketVersions,
#         s3:ListObjectsV2, s3:ListAllMyBuckets, s3:GetBucketLocation; any s3:Put*,
#         s3:Delete*, s3:Abort*, s3:Restore*, s3:*, or a `*` in the action is modify; any
#         other s3 action name is unknown;
#       * `none` alone: the credential was inspected and holds no storage permission;
#   - resources: a non-empty list of scopes, each one of
#       `*`, `account:*`, `account:<id>`, `bucket:*`, `bucket:<name>`,
#       `arn:aws:s3:::*`, `arn:aws:s3:::<bucket>`, `arn:aws:s3:::<bucket>/<key-prefix>`
#       (a `*` may end the bucket name or key prefix), or `none` alone with `none`
#       permissions.
# A scope covers the backup bucket when it is `*`, account-wide, `bucket:*`, or a bucket or
# ARN whose bucket part matches the bucket (wildcards included, any key prefix included).
# Any modify permission on a scope that covers the bucket is a VIOLATION (exit 4).
#
# Exit codes: 0 clean; 2 invalid input (unreadable, malformed, missing fields, or a value
# that looks like a secret, which is never echoed); 3 incomplete or unknown (blocked);
# 4 violation (a credential can remove objects in the bucket).
set -Eeuo pipefail
umask 077
# shellcheck source=scripts/m8/lib-secrets.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib-secrets.sh"

INV="${1:-}"
[ -n "$INV" ] && [ -f "$INV" ] || { echo "usage: check-r2-delete-rights.sh INVENTORY.json [evidence-file]" >&2; exit 2; }
EVIDENCE="${2:-.local/m8-evidence/r2-delete-rights-$(date -u +%Y%m%dT%H%M%SZ).md}"

# Every string value in the file, one per line (newlines escaped), for secret screening.
leaves="$(python3 -I - "$INV" <<'PY' 2>/dev/null
import json, sys
def walk(v):
    if isinstance(v, dict):
        for k, x in v.items():
            yield str(k)
            yield from walk(x)
    elif isinstance(v, list):
        for x in v:
            yield from walk(x)
    elif isinstance(v, str):
        yield v
try:
    data = json.load(open(sys.argv[1]))
except Exception:
    sys.exit(2)
for s in walk(data):
    print(s.replace("\\", "\\\\").replace("\n", "\\n"))
PY
)" || { echo "check-r2-delete-rights: the inventory is not valid JSON" >&2; exit 2; }
while IFS= read -r leaf; do
  if m8_looks_secret "$leaf"; then
    echo "check-r2-delete-rights: the inventory contains a value that looks like a secret; remove it (names and permission groups only)" >&2
    exit 2
  fi
done <<< "$leaves"

# The verdict: python prints the report and exits 0 clean, 2 invalid, 3 incomplete, 4 violation.
report="$(python3 -I - "$INV" <<'PY'
import fnmatch, json, re, sys
REQUIRED_SOURCES = ["env", "aws_profiles", "rclone", "wrangler", "cf_cli", "mounted_secrets", "other"]
TOP_KEYS = {"bucket", "environment", "inventory_complete", "sources_checked", "credentials"}
CRED_KEYS = {"name", "kind", "inspected", "permissions", "resources"}
KINDS = {"cloudflare_api_token", "r2_access_key", "aws_profile", "rclone_remote",
         "mounted_secret", "environment_secret"}
CF_READ = {"Workers R2 Storage Read", "Workers R2 Storage Bucket Item Read",
           "Account Settings Read", "Zone Read", "DNS Read", "Workers Scripts Read",
           "Workers KV Storage Read", "Account Analytics Read"}
CF_MODIFY = {"Workers R2 Storage Write", "Workers R2 Storage Bucket Item Write",
             "Account Settings Write", "Zone Write", "DNS Write", "Workers Scripts Write",
             "Workers KV Storage Write", "Administrator", "Super Administrator"}
S3_READ = {"GetObject", "GetObjectVersion", "HeadObject", "ListBucket", "ListBucketVersions",
           "ListObjectsV2", "ListAllMyBuckets", "GetBucketLocation"}
S3_MODIFY_PREFIX = ("Put", "Delete", "Abort", "Restore")
NAME = re.compile(r"^[a-z0-9*][a-z0-9.*-]{0,62}$")
ACCOUNT = re.compile(r"^account:(\*|[A-Za-z0-9_-]{1,64})$")
BUCKET = re.compile(r"^bucket:(\*|[a-z0-9*][a-z0-9.*-]{0,62})$")
ARN = re.compile(r"^arn:aws:s3:::([a-z0-9*][a-z0-9.*-]{0,62})(/[^\s]{0,1024})?$")

def perm_class(p):
    """read, modify, none, or None for an unknown permission."""
    if p == "none":
        return "none"
    if p in CF_READ:
        return "read"
    if p in CF_MODIFY:
        return "modify"
    if p.startswith("s3:"):
        action = p[3:]
        if not action:
            return None
        if "*" in action:
            return "modify"
        if action in S3_READ:
            return "read"
        if action.startswith(S3_MODIFY_PREFIX):
            return "modify"
        return None
    return None

def covers(scope, bucket):
    """True when the scope covers the bucket, False when not, None when unknown."""
    if scope == "*":
        return True
    m = ACCOUNT.match(scope)
    if m:
        return True
    m = BUCKET.match(scope) or ARN.match(scope)
    if m:
        name = m.group(1)
        return fnmatch.fnmatchcase(bucket, name)
    if scope == "arn:aws:s3:::*":
        return True
    return None

data = json.load(open(sys.argv[1]))
out, problems, violations = [], [], []
def need(obj, key, kind):
    if key not in obj or not isinstance(obj[key], kind):
        print(f"invalid: missing or wrong type: {key}"); sys.exit(2)
    return obj[key]
if not isinstance(data, dict):
    print("invalid: the inventory must be an object"); sys.exit(2)
bucket = need(data, "bucket", str)
env = need(data, "environment", str)
complete = need(data, "inventory_complete", bool)
sources = need(data, "sources_checked", list)
creds = need(data, "credentials", list)
out.append(f"# R2 delete-rights check; bucket: {bucket}; environment: {env}")
extra = sorted(set(data) - TOP_KEYS)
if extra:
    problems.append("unknown top-level keys: " + ", ".join(extra))
if not NAME.match(bucket) or "*" in bucket:
    problems.append("bucket is not a valid bucket name")
missing = [s for s in REQUIRED_SOURCES if s not in sources]
if missing:
    problems.append("sources not checked: " + ", ".join(missing))
unknown_sources = [s for s in sources if s not in REQUIRED_SOURCES]
if unknown_sources:
    problems.append("unknown sources: " + ", ".join(map(str, unknown_sources)))
if not complete:
    problems.append("inventory_complete is false")
for i, c in enumerate(creds):
    if not isinstance(c, dict) or not isinstance(c.get("name"), str):
        print(f"invalid: credential {i} has no name"); sys.exit(2)
    name = c["name"]
    extra = sorted(set(c) - CRED_KEYS)
    if extra:
        problems.append(f"{name}: unknown keys: {', '.join(extra)}")
    if c.get("kind") not in KINDS:
        problems.append(f"{name}: unknown kind")
    perms, res = c.get("permissions"), c.get("resources")
    if c.get("inspected") is not True or not isinstance(perms, list) or not isinstance(res, list):
        problems.append(f"{name}: not inspected, or its permissions/resources are not listed")
        out.append(f"- {name}: UNKNOWN (not inspected)")
        continue
    if not perms or not res:
        problems.append(f"{name}: permissions and resources must both be listed")
        out.append(f"- {name}: UNKNOWN (empty permissions or resources)")
        continue
    if not all(isinstance(x, str) for x in perms + res):
        problems.append(f"{name}: permissions and resources must be strings")
        continue
    classes = {p: perm_class(p) for p in perms}
    unknown_p = [p for p, k in classes.items() if k is None]
    if unknown_p:
        problems.append(f"{name}: unknown permissions: {', '.join(unknown_p)}")
    if "none" in classes.values() and len(perms) != 1:
        problems.append(f"{name}: 'none' must be the only permission")
    if res == ["none"]:
        if perms != ["none"]:
            problems.append(f"{name}: scope 'none' needs permissions 'none'")
        scope_cov = {}
    else:
        scope_cov = {r: covers(r, bucket) for r in res}
        unknown_r = [r for r, v in scope_cov.items() if v is None]
        if unknown_r or "none" in res:
            problems.append(f"{name}: unknown scopes: {', '.join(unknown_r or ['none'])}")
    modify = [p for p, k in classes.items() if k == "modify"]
    covering = [r for r, v in scope_cov.items() if v]
    if modify and covering:
        violations.append(f"{name}: {', '.join(modify)} on {', '.join(covering)}")
        out.append(f"- {name}: CAN WRITE OR REMOVE OBJECTS in {bucket}: {', '.join(modify)}")
    elif unknown_p or any(v is None for v in scope_cov.values()):
        out.append(f"- {name}: UNKNOWN (unrecognised permission or scope)")
    else:
        out.append(f"- {name}: no delete-capable permission on {bucket} ({', '.join(perms)})")
for p in problems:
    out.append(f"- BLOCKED: {p}")
for v in violations:
    out.append(f"- VIOLATION: {v}")
verdict = "violation" if violations else ("incomplete" if problems else "clean")
out.append(f"Verdict: {verdict}")
print("\n".join(out))
sys.exit(4 if violations else (3 if problems else 0))
PY
)" && rc=0 || rc=$?
mkdir -p "$(dirname "$EVIDENCE")"
printf '%s\n' "$report" > "$EVIDENCE"; chmod 600 "$EVIDENCE"
printf '%s\n' "$report"
echo "Evidence written to $EVIDENCE (mode 0600)."
exit "$rc"
