---
title: gateway
sidebar_position: 7
---

# `airs gateway`

`airs gateway` (the TypeScript CLI's `aigateway`) manages and observes Prisma AIRS AI
Gateway: workspaces and scopes, telemetry, the inventory and admin guardrails, writes,
inference and the environment report.

## Reads

```sh
airs gateway workspaces list                       # the data plane: workspaces you are scoped to
airs gateway workspaces list --all --output json   # admin plane, active and archived
airs gateway workspaces get ws-main-a-349e0e --plane admin
airs gateway scopes list
airs gateway scopes get ws_production_bx7qw0 --output json
airs gateway telemetry cost --workspace "Main A" --days 30   # a display name: needs the admin grant too
airs gateway telemetry requests --workspace ws-main-a-349e0e --start 2026-10-01T00:00:00Z --end 2026-10-02T00:00:00Z
airs gateway telemetry group-by model --workspace ws-main-a-349e0e --columns cost,total_tokens
airs gateway telemetry logs list --workspace ws-main-a-349e0e --page-size 20 --current-page 1
airs gateway configs list --workspace 16f7e90d-382a-4e78-b577-1b01eb5f8297
airs gateway providers get 2c1e5f0a-7d43-4b8e-9a51-0f2d3c4b5a69 --output json
airs gateway integrations providers                # the catalog slugs
airs gateway audit-logs list --days 1 --reveal-sensitive
airs gateway organisations auth-settings get       # the selected tenant's TSG
airs gateway admin-guardrails list --page-size 50 --current-page 0
```

The gateway commands use the tenant's `gatewayDataEndpoint`, `gatewayAdminEndpoint` and
`iamEndpoint` (else the SDK defaults) with the management OAuth identity and the `x-tsg-id`
header. Every telemetry command resolves `--workspace` (a slug, UUID or unique display name)
over the data plane, then the admin plane's active and archived lists, and refuses a reference
nothing matches before any telemetry request. A slug or UUID the data plane lists needs
only the data-plane grant; a display name also reads the admin-plane lists (an exact slug there
wins over a name) and so needs the admin grant too, and a 403 on those lists says to use the
slug or UUID. `--start` and `--end` take ISO-8601 times
with `Z` or an offset and are sent in UTC whatever the host's time zone; `--days` with `--start`
is refused. Every argument check runs before the tenant is read (exit 2). `table`
prints a table and an empty json list prints `[]`; acknowledgements, warnings and the 403
grant hint go to stderr, and a list the service says is incomplete is printed with a
warning. Every gateway document goes through one set of rules in every format: the SDK's secret paths for the resource; members whose names are credential
names in any case or separator (`clientSecret`, `API KEY`, `api.key`, `db_pwd`, `db_pass`,
`GITHUB_PAT`, `refreshTokens`, `tokenvalue`, `account_key`, `access_keys`, `basic_auth`,
`signature`, `passcode`, `otp`, a Vault `secret_id`, any name carrying a credential word such as
`adminpassword`: every name TypeScript masks but listed identifiers, references, token counts, key
names and lists of them (`metadataKeys`, `virtual_keys`), guardrail settings and `tokenizer`, a secret mapping's `secret_key`, and a name whose last
words describe a credential, such as `user_api_key_count`, `user_api_key_ttl`, `api_key_name`,
`token_type`, `token_expires_at` or `token_url`, which print); the values of header maps (`headers` and every `*_headers` name) and of
`{key, value}`, `{name, value}` and `{header, value}` pairs (header and environment lists) but the
safe headers'; the SDK's credential members; credentials inside a JSON text, and the whole of a text
that is shaped as JSON but does not parse; PEM private keys under any name; the `filter-boundaries`
data; and any string that carries a value withheld elsewhere. Each is printed as `[withheld]`.
Inside any string, under any name, the text rules recognise exactly these shapes:

- URLs: the userinfo, query and fragment of every `scheme://` URL, at the start of the string or
  inside prose (`connect ECONNREFUSED https://[withheld]@db/x?[withheld]`); the userinfo runs to the
  URL's last `@`, so a password holding `/`, `?` or `#` is withheld whole.
- Authorization schemes: the word after `Basic`, `Bearer` or `Token` when it looks like a
  credential or a password (or, after `Basic`, is base64 of `user:password`), also with the scheme
  joined to its header name (`Authorization: Bearer [withheld]`, `Authorization:Bearer [withheld]`).
- Header lines: the value after a credential header name written with a `-` (or `Authorization`,
  `Cookie`) and a `:` (`curl -H 'X-Api-Key: [withheld]'`, `-H "api-key:[withheld]"`,
  `Authorization: [withheld]`).
- `key=value` forms: the value of a credential-named key (`Password=[withheld];`,
  `client_secret=[withheld]`, `--token=[withheld]`, `SharedAccessSignature=[withheld]`), and a
  token-like value under `token=` or `key=`. In a connection string a credential's value runs to the
  next `;` (`Password=[withheld];` for `Password=p@ss w0rd;`) and an ODBC braced value to its closing
  `}` (`PWD=[withheld];` for `PWD={p@ss;w0rd};`); spaces around the `=` are read after a credential
  name only (`password = [withheld]`, `aws_secret_access_key = [withheld]`).

A long word after the ordinary words `token` or `basic` in prose (`token gpt-4o-mini-2024-07-18
quota exceeded`) is withheld in that text only, so the same model id elsewhere stays printed; so is a
credential key's value that only says it is absent or masked, or is a short word of letters and
digits (`auth=none`, `Password=true`, `Pwd=****`, `auth=oauth2`), which never withholds `none`,
`true` or `oauth2` in another row. A credential name's value that is one short word (under 8
characters of letters, digits and `-`, `_`, `.`, `/` or `:`: `Bearer`, `hunter2`), a placeholder or
mask, a state, mode or grant-type word (`disabled`, `required`, `anonymous`,
`oauth2_client_credentials`), a credential kind repeated by a sibling `*_type` or `*_mode`
(`{"auth": "password", "auth_type": "password"}`), or a timestamp is withheld in place only, so the
same word elsewhere is printed; a longer value (`dragonfly`) is withheld wherever it is echoed. A number and an empty string under a
credential name, `key`, `token` or a credential header (`{"password": 12345678}`,
`{"headers": {"Authorization": 12345678}}`, `{"key": "password", "value": 12345678}`) are withheld,
as TypeScript masks them on the inventory reads, and a number of 8 or more digits is withheld
wherever it is echoed. `null` and booleans are printed under every name, and so are numbers under
token-count names and other headers (`prompt_tokens: 5`, `x-retries: 3`) and price or usage
objects under token-count names (`prompt_tokens`, `request_token`); an object under a credential
name is withheld whole. Names that describe a credential print their values (`user_api_key_count`,
`user_api_key_ttl`, `token_type`, `token_expires_at`), and so do fingerprints (`api_key_last8`,
`api_key_hint`, `token_prefix`) unless the value looks like a credential itself. Model output (`inference`) gets the URL and scheme rules only. The
rules do not recognise, and print: a bare secret under a neutral member name with none of the
shapes above around it; a `name: value` line whose name is not a credential header
(`password: hunter2x`); `key = value` with spaces under a name that is not a credential name
(`token = …`); and a credential in prose with no scheme, header or `=` before it. Review output
before sharing it.

The inventory reads print TypeScript's generic tables (`Field │ Value` for one record) with those
rules applied: API key material, provider keys and model configs, integration and MCP secrets and
custom headers, SCIM tokens and SSO client secrets, deployment credentials and environment lists,
plugin credentials, service-account JSON, OAuth metadata and secret-manager settings at any depth,
the provider keys and service accounts inside a config's `config` text, and the
`organisations info` settings are withheld; `masked_*` fingerprints are printed. The
guardrail catalog prints its parameter schemas as sent, but for a non-empty `default`, `const`,
`enum` or `examples` under a credential-named parameter (an empty one, `""` or `[]`, prints).
`--reveal-sensitive` is refused on `api-keys`, `providers get` and `auth-settings get`; on
`audit-logs list` it shows the request body, headers and query as JSON with their credentials
still withheld, a secret reference's whole `auth_config` included (they are withheld by default). `--tsg-id` defaults to the selected
tenant's TSG and another value is refused. Ids are checked as UUIDs before the tenant; `integrations models list` lists the models TypeScript's generic list never showed.

## Writes

```sh
airs gateway workspaces create --name Production --scope-name ws_production_bx7qw0
airs gateway workspaces update Production --description 'Production workloads, us-east'
airs gateway workspaces archive ws-produc-985697 --force
airs gateway scopes bind ws_production_bx7qw0 --workspace ws-produc-985697
airs gateway configs create --name routing --workspace <uuid> --file config.json --set config.retry.attempts=3
airs gateway api-keys service create --name ci --scopes completions.write --type service --workspace <uuid> --secret-output ./ci-key.json
airs gateway api-keys service rotate <id> --secret-output ./rotated.json
airs gateway integrations create --ai-provider openai --name openai-prod --slug openai-prod --key-stdin < provider.key
airs gateway integrations workspaces set <id> --workspace-binding ws-main-a-349e0e=true --preserve-existing
airs gateway deployments update <id> --rotate-auth true --secret-output ./edge-credentials.json
airs gateway admin-guardrails mcp-servers sync <guardrail-id> --file mappings.json --force
```

Every write is sent once: a 401 is replayed once after a fresh token, a 403 never, and a write
whose outcome is unknown (a 5xx, a timeout, a dropped connection) or whose answer cannot be read
says so with a hint, and nothing is re-sent. `workspaces create` creates the IAM scope,
the workspace, the binding and reads the workspace back; it deletes the scope it made only after
a rejected workspace create and names every partial state. Request bodies come from
`--file` (one JSON object, YAML refused) < the named flags < `--set`/`--set-string`; no
error echoes a value; `--organisation-id` and `--tsg-id` default to the selected tenant's
TSG and another value is refused. One-time credentials (new API keys, rotated keys,
deployment credentials) go only to a new 0600 `--secret-output` file, reserved before the
request and never overwritten; `--show-secret` is refused. Deletes, archives (a
`deployments update --status archived` too), rotations, replacements, tenant-wide updates and
updates that replace a resource's authentication (a deployment's `--auth-settings`, an MCP
integration's `--auth-type` or credentials) ask for confirmation (`--force` skips it; without a terminal they are refused); deletes print nothing on stdout and every write acknowledges on stderr. Provider credentials come from `--key-file`, piped `--key-stdin` or a hidden prompt;
inline `--key`, `--scim-token`, `--credential` and `--header` values are accepted with a warning. An interrupted write says what may have changed.

## Previewing deployment updates and service-key creation

`gateway deployments update` and `gateway api-keys service create` support
`--dry-run`. They validate the effective request locally, resolve the selected
tenant's organisation and print a redacted preview. Management credentials are
not required for preview. No authentication, service request, confirmation prompt
or credential-file reservation occurs. Local tenant metadata and input files are
still read, and `--debug` can write its private diagnostic log.

```sh
airs --tenant example gateway deployments update <id> --set auth_settings.allow_all_workspaces=true --dry-run --output json
airs --tenant example gateway api-keys service create --name ci --workspace <uuid> --scopes completions.write --type <credential-type> --dry-run --output json
```

The preview reports execution requirements, including confirmation and a
credential destination when applicable. A key preview can omit `--secret-output`;
execution requires it. A supplied destination is checked without being reserved
or overwritten. Deployment preview refuses `--force`. Exit 0 means local
preparation succeeded; service acceptance and authorization remain unchecked.
The redacted preview is descriptive output, not a request file for execution.

Key creation requires `type` after file/flag/assignment merging. Supply the type
through `--type`, `--file`, or `--set`; help does not make a flag mandatory when
another input source supplies its value. The pinned local contract accepts a
nonempty string; use the creation type documented for your service. Expiration
timestamps support seconds with or without a fractional part, and alert emails
are optional.

Preview does not need management client credentials, but the selected tenant
still needs a configured `mgmtTsgId` matching its registered organisation identity.
Removing that value from a registered tenant causes the existing identity-pin
refusal before request preparation.

Preview shows known type, status and scope literals, numeric organisation IDs,
workspace UUIDs, valid expiry timestamps and well-formed `ws-` workspace slugs.
Workspace slugs may identify your project or customer. Other type/scope strings,
arbitrary labels, configuration, credentials and dynamic keys are withheld;
`redacted: true` indicates that the projection or shared policy withheld data.
Registered secrets are scrubbed from the entire preview. Validate the actual
service vocabulary against its documentation; preview does not establish service
acceptance. Displayable type/scope tokens are a fixed privacy policy, not a
service-support list. New vocabulary stays withheld until that display policy is
reviewed; it does not change local schema acceptance.

A zero dry-run exit status means local preparation succeeded. It does not mean
normal execution is ready: consumers must inspect `execution_requirements`,
supply any required secret-output destination and confirmation, and satisfy the
normal credential/grant requirements before executing.

Deployment authentication read and write representations differ. The pinned
write contract requires a JSON boolean for `allow_all_workspaces`; it rejects
`disable_portkey_gateway` and `grouped_gateway_base_url`. A complete GET record
is not a PUT request body. `--set` constructs the outgoing body and does not
fetch or merge the stored authentication settings. Establish the service's
replacement/merge behavior before choosing which writable settings to omit.

Typed-schema validation and selected-tenant binding refusals in gateway writes,
after debug-log creation, record a `validation_failure` lifecycle event with a static operation name, failure stage (`request_validation`
or `tenant_binding`) and preparation checkpoint. `requestSent: false` refers to
the refused mutation; provider resolution may already have made a read. This is
distinct from HTTP traffic and contains no request body. Argument-parser failures
before debug-log creation remain stderr-only errors. File/destination, credential,
confirmation, malformed input, config-load and missing management-client credential refusals
are reported on stderr outside this lifecycle adapter. If static operation text
matches a registered secret, the log uses a body-free `redactionFallback` record
without operation/stage details, or a generic marker when needed. Every fallback
passes the same privacy check. If every safe marker also matches a secret, the
record is suppressed; confidentiality remains authoritative.

In zsh, store repeated options in an array, for example
`tenant_args=(--tenant example)`, and invoke `airs "${tenant_args[@]}" ...`.
Use `airs doctor` to identify conflicting install channels. npm's launcher selects
its bundled executable; an identically versioned Homebrew executable can have
different bytes. Record the executable path and hash when comparing behavior.

## Inference and the report

```sh
airs gateway inference chat "Summarise our retry policy"          # the text
airs gateway inference chat --file request.json --model openai__gpt-x --output json
airs gateway inference responses "Hi" --stream                     # text as it arrives
airs gateway inference responses "Hi" --stream --output json       # one event per line (JSONL)
airs gateway inference embeddings "hello" --dimensions 256 --encoding float
airs gateway report --workspace ws-main-a-349e0e --output-file ./gateway-daily.html
airs gateway dashboard --workspace "Main A" --days 7 --strict --output-file - > weekly.html
```

`gateway inference` calls the runtime gateway with the tenant's `gatewayInferenceApiKey` as
`x-portkey-api-key` (no OAuth) on `--endpoint` or the tenant's `gatewayInferenceEndpoint`; the
model is `--model`, else the request file's, else `gatewayInferenceModel` (`gatewayEmbeddingModel`
for embeddings). No environment variable is read. Each call is sent once (model calls are
billed); `--timeout` bounds the whole request, the stream included. Model text is printed with its
line breaks and tabs and every other control character replaced, a registered secret withheld even
when split across stream events; `table` prints the answer's top level, `yaml` is refused; an
interruption says the request may have been processed and billed, exit 130/143.

`gateway report` (alias `dashboard`) reads 25 sources over one fixed window (the last `--days`, or
`--start`/`--end` with a time zone, sent in UTC): the tenant's workspaces, 17 telemetry charts, the
filter boundaries, the transaction logs page by page (`--max-pages`), the workspace's configs and
keys, the organisation and the guardrail catalog. It writes one self-contained HTML file (the red
team report's template: strict Content Security Policy, works offline) that is new, 0600 and never
overwritten, the default `airs-gateway-report-<time>-<id>.html`. It keeps aggregates only:
no workspace, key, user, model or URL. The destination and its directory are checked before any
request; `--output` and `--debug` are refused; a window outside the years
0000–9999 and zone-less times are refused. It exits 1 when no telemetry source could be
read, or with `--strict` when a source is incomplete.

## Every flag

Every command and flag is in the [command reference](../reference/commands/gateway.md).
