#!/usr/bin/env bash
# M8.3 step 1 helper: derive every value airs-mcp.toml needs, interactively.
#
#   scripts/m8/derive-config.sh [--talos-cluster DIR] [--spike FILE] [--prefill FILE] [--out FILE]
#
# For each value it explains what the value is, shows the exact read-only command that
# finds it when it already exists, and proposes a suggestion. Suggestions come from your
# local M0.6 live config (--spike, default prototypes/runtime-mcp/deploy/m0.6/
# spike-config.live.toml) when that file exists. Press Enter to accept a suggestion, or
# type or paste another value. Pasted values are screened (scripts/m8/lib-secrets.sh):
# anything that looks like a credential is refused and never echoed.
#
# OFFLINE ONLY (M8 review 3). This helper contacts no service and runs no service CLI
# (az, kubectl, cf, airs, mise, curl, wget) under any option. It reads only local files:
# the M0.6 config above, deploy/airs-mcp/, and your talos-cluster checkout
# (--talos-cluster, default ~/development/talos-cluster) for the CoreDNS, pihole and
# gateway entries. For every value it prints the exact read-only command(s) for YOU to run
# separately, under your own credentials, and accepts the sanitized result you paste (or
# the suggestion). A --prefill FILE (default .local/airs-mcp/prefill.toml when it exists)
# is a local, non-secret TOML of suggestions gathered beforehand (the same key paths as
# the spike config, plus scanner.{name,applicationId,endpoint,revision},
# management_revision and revision_label); it takes precedence over the spike config and
# is screened like any pasted value. It never creates, changes or deletes anything in Entra, the cluster,
# pihole or Cloudflare. The only program it runs is this repository's own `airs-mcp
# check-config`, which validates the output locally.
#
# Writes (only after the output validates as TOML, structurally, and with
# `airs-mcp check-config` when an airs-mcp binary is available):
#   .local/airs-mcp/airs-mcp.local.toml  (0600, gitignored; real identifiers, no secrets)
#   .local/airs-mcp/dns/*                (proposed talos-cluster DNS changes, if any)
# Exit status: 0 written and valid; 1 the derived configuration did not validate (nothing
# written); 2 usage.
set -uo pipefail

REPO=$(cd "$(dirname "$0")/../.." && pwd)
TALOS="${HOME}/development/talos-cluster"
OUT="$REPO/.local/airs-mcp/airs-mcp.local.toml"
SPIKE="$REPO/prototypes/runtime-mcp/deploy/m0.6/spike-config.live.toml"
PREFILL="$REPO/.local/airs-mcp/prefill.toml"
TEMPLATE="$REPO/deploy/airs-mcp/airs-mcp.toml"
while [ $# -gt 0 ]; do
  case "$1" in
    --run-lookups)
      echo "--run-lookups was removed: this helper is offline only; run the printed commands yourself" >&2
      exit 2 ;;
    --talos-cluster) TALOS=$2; shift 2 ;;
    --spike) SPIKE=$2; shift 2 ;;
    --prefill) PREFILL=$2; shift 2 ;;
    --out) OUT=$2; shift 2 ;;
    -h|--help) sed -n '2,32p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
DNSDIR="$(dirname "$OUT")/dns"

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
dim() { printf '\033[2m%s\033[0m\n' "$*"; }
warn() { printf '\033[33m! %s\033[0m\n' "$*"; }
ok() { printf '\033[32m✓ %s\033[0m\n' "$*"; }
have() { command -v "$1" >/dev/null 2>&1; }

# Refuse anything that looks like a credential rather than an identifier: the shared
# M8 detector, except the gateway's public dynamic-registration client IDs (pk_ + 64 hex),
# which are identifiers this file must hold.
# shellcheck source=scripts/m8/lib-secrets.sh
. "$REPO/scripts/m8/lib-secrets.sh"
looks_secret() {
  [[ $1 =~ ^pk_[0-9a-f]{64}$ ]] && return 1
  m8_looks_secret "$1"
}

# Suggested values: the prefill file first, then the local M0.6 live config (public
# identifiers only). A suggestion that looks like a credential is dropped.
spike() {
  local v=""
  if [ -f "$PREFILL" ]; then v=$(toml_get "$PREFILL" "$1"); fi
  if [ -z "$v" ] && [ -f "$SPIKE" ]; then v=$(toml_get "$SPIKE" "$1"); fi
  if [ -n "$v" ] && looks_secret "$v"; then return 0; fi
  printf '%s' "$v"
}
toml_get() {
  python3 -I - "$1" "$2" <<'PY' 2>/dev/null
import sys
def _toml():
    try:
        import tomllib
        return tomllib
    except ImportError:
        try:
            import tomli
            return tomli
        except ImportError:
            return None
toml = _toml()
def _mini(text):
    # Fallback without tomllib/tomli (CI's Python 3.10): the subset these files use —
    # `key = "string"`, `key = ["a", "b"]`, `[table]`, `[[array-of-tables]]`, comments.
    import re
    data = {}
    data_cur = data
    for raw in text.splitlines():
        line = raw.split("#", 1)[0].strip() if not raw.strip().startswith("#") else ""
        if not line:
            continue
        m = re.fullmatch(r"\[\[([A-Za-z0-9_.-]+)\]\]", line)
        if m:
            data_cur = {}
            data.setdefault(m.group(1), []).append(data_cur)
            continue
        m = re.fullmatch(r"\[([A-Za-z0-9_.-]+)\]", line)
        if m:
            data_cur = data.setdefault(m.group(1), {})
            continue
        m = re.fullmatch(r'([A-Za-z0-9_-]+)\s*=\s*(.*)', line)
        if not m:
            continue
        key, val = m.group(1), m.group(2).strip()
        if val.startswith("["):
            data_cur[key] = re.findall(r'"((?:[^"\\]|\\.)*)"', val)
        elif val.startswith('"'):
            mm = re.match(r'"((?:[^"\\]|\\.)*)"', val)
            data_cur[key] = mm.group(1) if mm else ""
        elif val in ("true", "false"):
            data_cur[key] = val == "true"
        else:
            data_cur[key] = val
    return data
if toml is None:
    data = _mini(open(sys.argv[1], encoding="utf-8").read())
else:
    data = toml.load(open(sys.argv[1], "rb"))
cur = data
for part in sys.argv[2].split("."):
    if isinstance(cur, list):
        cur = cur[int(part)] if part.isdigit() and int(part) < len(cur) else None
    elif isinstance(cur, dict):
        cur = cur.get(part)
    if cur is None:
        break
if isinstance(cur, list):
    print(",".join(str(x) for x in cur))
elif cur is not None:
    print(cur)
PY
}

# ask VAR "question" "suggestion" [validator] [how-to-find lines...]
ask() {
  local var=$1 question=$2 suggestion=$3 validator=${4:-any}
  shift 4 || shift $#
  echo
  bold "$question"
  local line
  for line in "$@"; do dim "  $line"; done
  [ $# -eq 0 ] || dim "  (run the command above yourself and paste its result)"
  local answer
  while true; do
    if [ -n "$suggestion" ]; then
      IFS= read -r -p "  value [$suggestion]: " answer || answer=""
      answer=${answer:-$suggestion}
    else
      IFS= read -r -p "  value: " answer || { warn "no input"; exit 1; }
    fi
    answer=$(printf '%s' "$answer" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')
    if [ -z "$answer" ]; then
      warn "a value is required"; continue
    fi
    if [[ $answer =~ [[:cntrl:]] ]]; then
      warn "control characters are not allowed; try again"; continue
    fi
    if looks_secret "$answer"; then
      warn "that looks like a credential; this file holds identifiers only. Try again."
      continue
    fi
    if ! validate "$validator" "$answer"; then
      continue
    fi
    printf -v "$var" '%s' "$answer"
    return 0
  done
}

uuid_re='^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$'
validate() {
  local kind=$1 v=$2 item
  case "$kind" in
    any) return 0 ;;
    uuid)
      [[ $v =~ $uuid_re ]] && [[ $v != 00000000-0000-4000-8000-* ]] && return 0
      warn "expected a real GUID (not the template's 00000000-… placeholder)"; return 1 ;;
    uuids)
      IFS=',' read -ra items <<<"$v"
      for item in "${items[@]}"; do
        item=${item// /}
        [[ $item =~ $uuid_re ]] || { warn "not a GUID: $item"; return 1; }
      done
      return 0 ;;
    host)
      [[ $v =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$ ]] && return 0
      warn "expected a lowercase DNS name"; return 1 ;;
    url)
      [[ $v =~ ^https://[^[:space:]\"]+$ ]] && return 0
      warn "expected an https:// URL"; return 1 ;;
    digits)
      [[ $v =~ ^[0-9]{6,}$ ]] && return 0
      warn "expected the numeric TSG ID"; return 1 ;;
    email)
      [[ $v =~ ^[^@[:space:]\"]+@[^@[:space:]\"]+\.[a-z]{2,}$ ]] && return 0
      warn "expected the email address the gateway asserts for you"; return 1 ;;
    list) return 0 ;;
  esac
  return 0
}

[ -t 1 ] && clear
bold "airs-mcp configuration: derive every value for deploy/airs-mcp/airs-mcp.toml"
echo "Output: $OUT (0600, gitignored). Nothing here is a secret; nothing is changed remotely."
ok "offline: no service is contacted; each lookup command is shown for you to run"
[ -f "$SPIKE" ] && ok "suggestions come from your M0.6 live config ($SPIKE)" \
  || warn "no M0.6 live config found; suggestions are defaults"

# ---------------------------------------------------------------------------------
bold $'\n== 1/7  Service hostname and DNS'
echo "The gateway calls the backend inside the cluster at this hostname. Clients never"
echo "resolve it: they use the gateway (mcp.goshawks.io). It must equal the certificate's"
echo "dnsNames and the gateway integration's backend URL."
s_host=$(spike server.allowed_hosts | cut -d, -f1)
ask HOST "Backend hostname" "${s_host:-mcp-backend-airs.goshawks.io}" host \
  "Find: grep -n 'mcp-backend-airs' $TALOS/coredns/configmap.yaml" \
  "Suggested: mcp-backend-airs.goshawks.io (already wired for the M0.6 spike)"
RESOURCE="https://$HOST/mcp"

echo
bold "Checking where $HOST must and must not resolve (local files only)…"
mkdir -p "$DNSDIR"
if [ -f "$TALOS/coredns/configmap.yaml" ]; then
  if grep -qF "rewrite name exact $HOST airs-mcp.airs-mcp.svc.cluster.local" "$TALOS/coredns/configmap.yaml"; then
    ok "CoreDNS rewrite present ($HOST → airs-mcp.airs-mcp.svc.cluster.local)"
  else
    warn "no CoreDNS rewrite for $HOST: writing $DNSDIR/coredns.patch-snippet"
    cat >"$DNSDIR/coredns.patch-snippet" <<EOF
    # airs-mcp backend for the goshawks gateway (TLS on the Service; never ingress).
    $HOST:53 {
        errors
        cache 30
        rewrite name exact $HOST airs-mcp.airs-mcp.svc.cluster.local
        kubernetes cluster.local in-addr.arpa ip6.arpa {
            ttl 30
        }
        forward . 1.1.1.1 1.0.0.1
    }
EOF
    dim "  add it to $TALOS/coredns/configmap.yaml beside the mcp-backend-bonbon block"
  fi
  if grep -qF "$HOST" "$TALOS/aigw-goshawks/values.yaml" 2>/dev/null \
    && grep -q "TRUSTED_CUSTOM_HOSTS" "$TALOS/aigw-goshawks/values.yaml"; then
    ok "gateway trusts $HOST (aigw-goshawks TRUSTED_CUSTOM_HOSTS)"
  else
    warn "add $HOST to TRUSTED_CUSTOM_HOSTS in $TALOS/aigw-goshawks/values.yaml"
  fi
  for d in dns1 dns2; do
    if grep -qF "$HOST" "$TALOS/pihole/$d/values.yaml" 2>/dev/null; then
      warn "pihole $d has a record for $HOST: LAN clients would bypass the gateway; consider removing it"
    else
      ok "pihole $d: no LAN record for $HOST (correct: it is reached only inside the cluster)"
    fi
  done
  for gw in mcp.goshawks.io gateway.goshawks.io; do
    if grep -qF "host-record=$gw," "$TALOS/pihole/dns1/values.yaml" 2>/dev/null \
      && grep -qF "host-record=$gw," "$TALOS/pihole/dns2/values.yaml" 2>/dev/null; then
      ok "pihole dns1 and dns2 resolve the gateway name $gw"
    else
      warn "pihole is missing $gw: writing $DNSDIR/pihole-$gw.patch-lines"
      echo "    - host-record=$gw,192.168.1.0" >"$DNSDIR/pihole-$gw.patch-lines"
      dim "  add the line to pihole/dns1/values.yaml, merge, verify with dig @192.168.1.1, then dns2"
    fi
  done
  if grep -q "cloudflare" "$TALOS/talos/cluster-issuer.yaml" 2>/dev/null; then
    ok "letsencrypt-prod solves dns01 through Cloudflare"
  fi
else
  warn "talos-cluster not found at $TALOS (use --talos-cluster DIR); DNS checks skipped"
fi

ZONE=${HOST#*.}
echo
echo "Public DNS (Cloudflare, zone $ZONE): the backend needs NO public record. cert-manager's"
echo "letsencrypt-prod issuer proves the name with a temporary DNS-01 TXT record it creates"
echo "itself through its Cloudflare token."
dim "  Check (read-only): cf dns records list --zone $ZONE --name $HOST"
dim "  Expected result: no A, AAAA or CNAME record for $HOST. If one exists, the backend is"
dim "  public; review it in the dashboard."
dim "  This helper never creates DNS records. If you deliberately want a public record (normally"
dim "  NOT needed, and it would expose the backend), create it yourself, e.g.:"
dim "    cf dns records create --zone $ZONE --type A --name $HOST --content <ip> --ttl 300"

# ---------------------------------------------------------------------------------
bold $'\n== 2/7  Entra (the upstream token the gateway presents)'
s_tid=$(spike upstream.tenant_id)
ask TENANT_ID "Entra tenant ID" "$s_tid" uuid \
  "Find: az account show --query tenantId -o tsv" \
  "Or: Entra admin center → Overview → Tenant ID"
s_api=$(spike upstream.audiences | cut -d, -f1)
ask API_APP "airs-mcp API application (client) ID: the upstream token's audience" "$s_api" uuid \
  "Find (names vary, e.g. AIRS-MCP-API; match case-insensitively):" \
  "  az ad app list --all --query '[].{name:displayName,appId:appId}' -o tsv | grep -i 'airs'" \
  "Check its App ID URI is $RESOURCE: az ad app show --id <appId> --query 'identifierUris[0]' -o tsv"
dim "  The App ID URI must be $RESOURCE (the scopes below use it):"
dim "    az ad app show --id $API_APP --query 'identifierUris[0]' -o tsv"
s_gw=$(spike upstream.allowed_client_ids)
ask GW_CLIENT "Gateway confidential client ID(s): the token's azp (comma-separated)" "$s_gw" uuids \
  "Find (the gateway's dedicated client; names vary, match case-insensitively):" \
  "  az ad app list --all --query '[].{name:displayName,appId:appId}' -o tsv | grep -i 'airs' | grep -i 'gateway'" \
  "This must be a DEDICATED confidential client used by this integration only (plan revision 7)."
s_own=$(spike upstream.owner_object_ids)
ask OWNERS "Owner object ID(s): who owns the service (comma-separated)" "$s_own" uuids \
  "Find yours: az ad signed-in-user show --query id -o tsv"

# ---------------------------------------------------------------------------------
bold $'\n== 3/7  Gateway identity assertion'
echo "The gateway signs an identity assertion (header x-user-jwt). These values come from"
echo "the gateway integration you provisioned for M0.6."
ask A_ISS "Assertion issuer" "$(spike assertion.issuer)" any \
  "Find: decode one assertion's 'iss' (M0.6 runbook step 5 recorded it)"
ask A_JWKS "Assertion JWKS URL" "$(spike assertion.jwks_url)" url \
  "Find: https://mcp.goshawks.io/.well-known/jwks.json (the gateway's published keys)"
ask A_AUD "Assertion audience(s): the organisation ID the gateway signs as aud" "$(spike assertion.audiences)" list \
  "M0.6 finding: aud = organisation ID (org-wide, not per-backend)"
ask A_ORG "Organisation ID(s)" "$(spike assertion.organization_ids)" list \
  "Find: gateway console → Organisation settings"
ask A_WS "Workspace ID(s) allowed to reach this backend" "$(spike assertion.workspace_ids)" list \
  "Find: gateway console → Workspaces (the workspace in https://mcp.goshawks.io/<workspace>/airs/mcp)"

# ---------------------------------------------------------------------------------
bold $'\n== 4/7  AIRS tenant'
s_tsg=$(spike airs.tsg_id); [ -n "$s_tsg" ] || s_tsg=$(spike service.tenant | sed -n 's/^airs-tsg-//p')
ask TSG "AIRS TSG ID" "$s_tsg" digits \
  "Find: airs tenant list --output json   (tsgId of the active tenant)"
ask MGMT_REV "Management credential revision (a GUID naming this credential version)" "$(spike management_revision)" uuid \
  "This is NOT secret: it names the credential version so collections can be attributed to it." \
  "Suggested: a new GUID for this deployment:  uuidgen   (rotate it whenever the secret changes)"
echo
PF_SC_NAME=$(spike scanner.name); PF_SC_EP=$(spike scanner.endpoint); PF_REV=$(spike revision_label)
bold "Scanner"
dim "  'Scanner' is this CLI's name for one AI Runtime Security API credential binding: the"
dim "  AIRS API key for an application (SCM → AI Runtime Security → Applications), its scan"
dim "  endpoint, a revision GUID and that application's ID. Create one from the flat key with"
dim "    airs tenant scanners bind-default --app-id <application id> <tenant> monitor"
dim "  Find: airs tenant scanners list --output json   (public metadata only)"
ask SC_NAME "Scanner name" "${PF_SC_NAME:-monitor}" any "Find: airs tenant scanners list (name)"
ask SC_APP "Scanner application ID (the AIRS application the key belongs to)" "$(spike scanner.applicationId)" any \
  "Find: airs tenant scanners list --output json (applicationId)" \
  "Or: airs runtime customer-apps list --output json --quiet (id / app_name)"
ask SC_EP "Scanner endpoint" "${PF_SC_EP:-https://service.api.aisecurity.paloaltonetworks.com}" url \
  "Find: airs tenant scanners list --output json (endpoint)"
ask SC_REV "Scanner credential revision (a GUID)" "$(spike scanner.revision)" uuid \
  "Find: airs tenant scanners list --output json (revision), or a new uuidgen when you rotate the key"

# ---------------------------------------------------------------------------------
bold $'\n== 5/7  Callers and grants'
echo "The gateway forwards your email as the subject (M0.6 finding: no sub)."
ask SUBJECT "Your caller subject (email as the gateway asserts it)" "$(spike grants.0.subject)" email \
  "Find: the 'email' claim of your assertion (M0.6 step 5)"
ask CLI_CLIENT "airs CLI public client ID (the client_id in the assertion)" "$(spike grants.1.client_id)" any \
  "Find: the Idira public client registered for the airs CLI (M0.6 step 2)"
ask CC_CLIENT "Claude Code client ID (gateway-issued, dynamic registration, starts pk_)" "$(spike grants.0.client_id)" any \
  "Find: gateway console → the Claude Code agent's client (it is NOT the Idira ID)"
dim "  Grant tools are copied from deploy/airs-mcp/airs-mcp.toml (by label), so the proofs'"
dim "  report and job tools stay in step with the template."

# ---------------------------------------------------------------------------------
bold $'\n== 6/7  Revision'
ask REVISION "Configuration revision label" "${PF_REV:-prod-$(date +%Y%m%d)-1}" any \
  "Any label; change it whenever this file changes (it is recorded with every job)."

# ---------------------------------------------------------------------------------
bold $'\n== 7/7  Writing the configuration'
umask 077
mkdir -p "$(dirname "$OUT")"
TMP=$(mktemp "$(dirname "$OUT")/.airs-mcp.toml.XXXXXX") || { warn "cannot create a temporary file"; exit 1; }
trap 'rm -f "$TMP"' EXIT
export D_REVISION="$REVISION" D_HOST="$HOST" D_RESOURCE="$RESOURCE" D_TENANT_ID="$TENANT_ID" \
  D_API_APP="$API_APP" D_GW_CLIENT="$GW_CLIENT" D_OWNERS="$OWNERS" D_A_ISS="$A_ISS" \
  D_A_JWKS="$A_JWKS" D_A_AUD="$A_AUD" D_A_ORG="$A_ORG" D_A_WS="$A_WS" D_TSG="$TSG" \
  D_MGMT_REV="$MGMT_REV" D_SC_NAME="$SC_NAME" D_SC_APP="$SC_APP" D_SC_EP="$SC_EP" \
  D_SC_REV="$SC_REV" D_SUBJECT="$SUBJECT" D_CLI_CLIENT="$CLI_CLIENT" D_CC_CLIENT="$CC_CLIENT" \
  D_TEMPLATE="$TEMPLATE"
# Every value is serialized as a TOML basic string by json.dumps (quotes, backslashes and
# control characters escaped), then the whole file is parsed back and checked.
if ! python3 -I - "$TMP" <<'PY'
import json, os, sys, datetime
def _toml():
    try:
        import tomllib
        return tomllib
    except ImportError:
        try:
            import tomli
            return tomli
        except ImportError:
            return None
toml = _toml()
e = os.environ
def s(v):
    return json.dumps(v, ensure_ascii=False)
def arr(v):
    items = [x.strip() for x in v.split(",") if x.strip()]
    return "[" + ", ".join(s(x) for x in items) + "]"
# Grant tools follow the template, by label; fallbacks are the proof-ready sets.
tools = {
    "claude-code": ["service_info", "history_status", "history_coverage", "history_query",
                    "reports_list", "reports_get", "artifacts_read", "artifacts_download",
                    "jobs_list", "jobs_get"],
    "airs-cli": ["service_info", "history_status", "history_coverage", "history_query",
                 "collection_start", "receipts_ingest", "reports_create", "reports_list",
                 "reports_get", "artifacts_read", "artifacts_download", "jobs_list",
                 "jobs_get", "schedules_list"],
}
try:
    tpl = toml.load(open(e["D_TEMPLATE"], "rb")) if toml else {}
    for g in tpl.get("grants", []):
        if g.get("label") in tools and isinstance(g.get("tools"), list):
            tools[g["label"]] = [str(t) for t in g["tools"]]
except Exception:
    pass
owner1 = e["D_OWNERS"].split(",")[0].strip()
tid = e["D_TENANT_ID"]
res = e["D_RESOURCE"]
now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
L = [
 f"# airs-mcp production configuration, derived by scripts/m8/derive-config.sh on {now}.",
 "# Real identifiers, no secrets. Gitignored: never commit (the repository is mirrored publicly).",
 f"revision = {s(e['D_REVISION'])}",
 "",
 "[server]",
 'listen = "0.0.0.0:8443"',
 f"allowed_hosts = [{s(e['D_HOST'])}]",
 f"resource = {s(res)}",
 f"authorization_servers = [{s('https://login.microsoftonline.com/' + tid + '/v2.0')}]",
 "scopes_supported = [",
 f"  {s(res + '/AirsHistory.Read')},",
 f"  {s(res + '/AirsHistory.Operate')},",
 f"  {s(res + '/AirsHistory.Admin')},",
 '  "offline_access",',
 "]",
 'data_dir = "/data/airs"',
 'health_listen = "0.0.0.0:8081"',
 "",
 "[server.tls]",
 'cert_file = "/etc/airs-mcp/tls/tls.crt"',
 'key_file = "/etc/airs-mcp/tls/tls.key"',
 "",
 "[upstream]",
 f"issuer = {s('https://login.microsoftonline.com/' + tid + '/v2.0')}",
 f"audiences = [{s(e['D_API_APP'])}]",
 f"tenant_id = {s(tid)}",
 f"jwks_url = {s('https://login.microsoftonline.com/' + tid + '/discovery/v2.0/keys')}",
 'algorithms = ["RS256"]',
 'accepted_permissions = ["AirsHistory.Read", "AirsHistory.Operate", "AirsHistory.Admin"]',
 f"allowed_client_ids = {arr(e['D_GW_CLIENT'])}",
 f"owner_object_ids = {arr(e['D_OWNERS'])}",
 "",
 "[assertion]",
 'header = "x-user-jwt"',
 f"issuer = {s(e['D_A_ISS'])}",
 f"audiences = {arr(e['D_A_AUD'])}",
 'algorithms = ["RS256"]',
 f"jwks_url = {s(e['D_A_JWKS'])}",
 "max_lifetime_seconds = 300",
 f"workspace_ids = {arr(e['D_A_WS'])}",
 f"organization_ids = {arr(e['D_A_ORG'])}",
 "",
 "[assertion.claims]",
 'subject = "email"',
 'client_id = "client_id"',
 'workspace = "workspace_id"',
 'organization = "organisation_id"',
 "",
 "[admin]",
 'mode = "disabled"',
 "",
 "[service]",
 f"tenant = {s('airs-tsg-' + e['D_TSG'])}",
 'timezone = "America/Chicago"',
 "",
 "[airs]",
 f"tsg = {s(e['D_TSG'])}",
 'management_endpoint = "https://api.apps.paloaltonetworks.com/aisec"',
 'dashboard_endpoint = "https://api.apps.paloaltonetworks.com/aisec"',
 'token_endpoint = "https://auth.apps.paloaltonetworks.com/oauth2/access_token"',
 'management_client_id_file = "/run/airs-mcp/secrets/mgmt-client-id"',
 'management_client_secret_file = "/run/airs-mcp/secrets/mgmt-client-secret"',
 f"management_revision = {s(e['D_MGMT_REV'])}",
 "",
 "[[airs.scanners]]",
 f"name = {s(e['D_SC_NAME'])}",
 f"applicationId = {s(e['D_SC_APP'])}",
 f"endpoint = {s(e['D_SC_EP'])}",
 "numRetries = 0",
 f"revision = {s(e['D_SC_REV'])}",
 f"api_key_file = {s('/run/airs-mcp/secrets/scanner-' + e['D_SC_NAME'])}",
 "",
 "[[receipt_associations]]",
 f"subject = {s(e['D_SUBJECT'])}",
 f"client_id = {s(e['D_CLI_CLIENT'])}",
 f"declared_scanner = {s(e['D_SC_NAME'])}",
 f"scanner = {s(e['D_SC_NAME'])}",
 "",
 "[limits]",
 "collection_interval_minutes = 15",
]
for label, client, perm in (("claude-code", e["D_CC_CLIENT"], "reader"), ("airs-cli", e["D_CLI_CLIENT"], "operator")):
    L += ["", "[[grants]]", f"subject = {s(e['D_SUBJECT'])}", f"client_id = {s(client)}",
          f"owner_object_id = {s(owner1)}", f"permission = {s(perm)}", f"label = {s(label)}",
          "tools = [" + ", ".join(s(t) for t in tools[label]) + "]"]
text = "\n".join(L) + "\n"
open(sys.argv[1], "w").write(text)
# Parse back, and check the structure the service needs. Without a TOML parser
# (Python < 3.11 and no tomli) the shell requires `airs-mcp check-config` instead.
if toml is None:
    sys.exit(0)
d = toml.loads(text)
need = {"revision": str, "server": dict, "upstream": dict, "assertion": dict, "admin": dict,
        "service": dict, "airs": dict, "grants": list, "receipt_associations": list}
for k, t in need.items():
    if not isinstance(d.get(k), t):
        sys.exit(f"structure: {k} missing or wrong type")
if d["revision"] != e["D_REVISION"]:
    sys.exit("structure: the revision did not round-trip")
if d["server"]["allowed_hosts"] != [e["D_HOST"]] or len(d["grants"]) != 2:
    sys.exit("structure: hosts or grants did not round-trip")
PY
then
  warn "the derived configuration did not validate; nothing was written"
  exit 1
fi
# Python 3.11+ (tomllib) or the tomli module gives the round-trip check above; without
# either, the service parser below is required rather than optional.
# AIRS_DERIVE_ASSUME_NO_TOML=1 makes the helper behave as if python3 had no TOML module
# (tests exercise that path on machines that have one). It can only make validation
# stricter: the service parser becomes mandatory.
if [ "${AIRS_DERIVE_ASSUME_NO_TOML:-0}" != 1 ] \
  && { python3 -I -c 'import tomllib' 2>/dev/null || python3 -I -c 'import tomli' 2>/dev/null; }; then
  HAVE_TOML=1; ok "it parses as TOML and has the expected structure"
else
  HAVE_TOML=0; dim "  (no TOML module in this python3: the service parser must validate it)"
fi
# The validator is the first EXECUTABLE candidate. An AIRS_MCP_BIN that is set but is not
# an executable file is an error, never silently replaced by a discovered binary and never
# counted as a validator (M8 review 6 finding 2).
VALIDATOR=""
if [ -n "${AIRS_MCP_BIN:-}" ] && ! { [ -f "${AIRS_MCP_BIN}" ] && [ -x "${AIRS_MCP_BIN}" ]; }; then
  warn "AIRS_MCP_BIN is set but is not an executable file; fix it or unset it. Nothing was written"
  exit 1
fi
for c in "${AIRS_MCP_BIN:-}" "$REPO/target/release/airs-mcp" "$REPO/target/debug/airs-mcp"; do
  if [ -n "$c" ] && [ -f "$c" ] && [ -x "$c" ]; then VALIDATOR=$c; break; fi
done
if [ -z "$VALIDATOR" ] && [ "$HAVE_TOML" = 0 ]; then
  warn "nothing can validate the output here: build airs-mcp or set AIRS_MCP_BIN to an executable; nothing was written"
  exit 1
fi
if [ -n "$VALIDATOR" ]; then
  if "$VALIDATOR" check-config --config "$TMP" >/dev/null 2>&1; then
    ok "airs-mcp check-config accepts it"
  else
    warn "airs-mcp check-config refused it; nothing was written:"
    "$VALIDATOR" check-config --config "$TMP" 2>&1 | sed 's/^/    /' | head -5
    exit 1
  fi
else
  dim "  (no airs-mcp binary found: build it, or set AIRS_MCP_BIN, to validate with the service parser)"
fi
chmod 600 "$TMP"
mv -f "$TMP" "$OUT"
trap - EXIT
ok "wrote $OUT (0600)"

if grep -nE '00000000-0000-4000-8000-|PLACEHOLDER|example' "$OUT" >/dev/null; then
  warn "placeholders remain:"; grep -nE '00000000-0000-4000-8000-|PLACEHOLDER|example' "$OUT" | sed 's/^/    /'
else
  ok "no template placeholders remain"
fi
cert=$(python3 -I -c 'import json,sys; print(",".join(json.load(open(sys.argv[1]))["spec"]["dnsNames"]))' "$REPO/deploy/airs-mcp/certificate.json" 2>/dev/null)
[ "$cert" = "$HOST" ] && ok "certificate.json dnsNames match $HOST" \
  || warn "deploy/airs-mcp/certificate.json dnsNames ($cert) differ from $HOST: update it"
ls "$DNSDIR"/* >/dev/null 2>&1 && { echo; bold "Proposed DNS changes (review, then merge in talos-cluster):"; ls -1 "$DNSDIR"; }

echo
bold "Next"
echo "  1. Load it as the ConfigMap the Deployment mounts (runbook step 6), e.g."
echo "       kubectl -n airs-mcp create configmap airs-mcp-config --from-file=airs-mcp.toml=$OUT --dry-run=client -o yaml"
echo "  2. Record step 1 as done in scripts/m8/user-checklist.sh (describe it; do not paste the file)."
echo "  3. Secrets (step 3) are separate: management client ID and secret, scanner API key, TLS."
exit 0
