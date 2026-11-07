# shellcheck shell=bash
# Shared credential detection for the M8 owner-run helpers (sourced, never executed).
# A text is refused when it looks like it carries a credential; the caller never echoes a
# refused text. Field kinds:
#   text    free text: refused on any credential sign below
#   hash    a SHA-256: accepts exactly `sha256:<64 lowercase hex>` or `<64 lowercase hex>`
#   status  one of: pass, fail, skip
#
# Credential signs (case-insensitive where it matters):
#   - a PEM block header (-----BEGIN ...);
#   - a JWT (eyJ... . ...);
#   - an HTTP Authorization value (Authorization: Bearer|Basic|Token ..., or a bare
#     "Bearer <value>"/"Basic <value>");
#   - a key-value pair naming a secret (password, passwd, pwd, secret, client_secret,
#     token, access_token, refresh_token, api_key, apikey, private_key, credential) with
#     a non-empty value after ':' or '=';
#   - an AWS access key ID (AKIA/ASIA + 16);
#   - a long high-entropy run: 32 or more of [A-Za-z0-9+_=] containing both a letter and
#     a digit. Well-formed `sha256:<64 hex>` digests and UUIDs are removed first, so an
#     image pin or a tenant ID in a result line is not mistaken for a credential, and '/'
#     and '-' split runs, so file paths are not either.

m8_looks_secret() {
  local s="$1" lower stripped run
  local re_jwt='eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}'
  local re_authz='authorization[[:space:]]*:[[:space:]]*[a-z]+[[:space:]]+[^[:space:]]+'
  local re_scheme='(^|[^a-z])(bearer|basic)[[:space:]]+[a-z0-9._~+/=-]{8,}'
  local re_kv="(password|passwd|pwd|secret|client_secret|token|access_token|refresh_token|api[_-]?key|apikey|private[_-]?key|credential)s?[\"']?[[:space:]]*[:=][[:space:]]*[\"']?[^[:space:]\"']+"
  local re_aws='(AKIA|ASIA)[0-9A-Z]{16}'
  lower="${s,,}"
  [[ "$s" == *"-----BEGIN"* ]] && return 0
  [[ "$s" =~ $re_jwt ]] && return 0
  [[ "$lower" =~ $re_authz ]] && return 0
  [[ "$lower" =~ $re_scheme ]] && return 0
  [[ "$lower" =~ $re_kv ]] && return 0
  [[ "$s" =~ $re_aws ]] && return 0
  stripped="$(printf '%s' "$s" | sed -E 's/sha256:[0-9a-f]{64}//g; s/[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}//g')"
  while read -r run; do
    [ -n "$run" ] || continue
    [[ "$run" =~ [A-Za-z] && "$run" =~ [0-9] ]] && return 0
  done < <(printf '%s' "$stripped" | grep -oE '[A-Za-z0-9+_=]{32,}' || true)
  return 1
}

# Validate `value` as field `kind`: 0 accepted, 3 refused (looks like a credential), 4
# invalid for the kind.
m8_check() {
  local kind="$1" value="$2"
  case "$kind" in
    hash)
      [[ "$value" =~ ^(sha256:)?[0-9a-f]{64}$ ]] && return 0
      return 4 ;;
    status)
      [[ "$value" =~ ^(pass|fail|skip)$ ]] && return 0
      return 4 ;;
    text)
      m8_looks_secret "$value" && return 3
      return 0 ;;
    *) return 4 ;;
  esac
}
