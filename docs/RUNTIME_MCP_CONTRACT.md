# Optional runtime MCP service contract (M0)

Status: **M0 contract, prepared 2026-11-04, revised 2026-11-05 with the M0.6 live
measurements**, for the [implementation plan](RUNTIME_MCP_IMPLEMENTATION_PLAN.md)
revision 7 (whose amendments await independent review). It freezes the package
boundary, schemas, identifiers and limits that M1–M8 implement. It does not claim
that any service, tool, command or flag exists. The M0.6 live gate
([runbook](RUNTIME_MCP_M0_6_RUNBOOK.md)) **was executed**; its outcome is §12.1, and M0
is **not accepted** until an independent review of the combined record passes. Loopback evidence for
the choices below is in [the M0 feasibility record](RUNTIME_MCP_M0_FEASIBILITY.md).
Where M0.6 evidence can change a value, the value is marked **(M0.6)**. Changing a
frozen item after M0 acceptance requires a recorded contract revision and review.

Sections: 1 packages and pins · 2 identity and authorization · 3 administrative
authority · 4 tool protocol · 5 response and transfer bounds · 6 limits and quotas ·
7 optional CLI · 8 scheduling · 9 storage, recovery and ownership · 10 jobs ·
11 server configuration · 12 prerequisites and decision rules.

## 1. Packages, dependency pins and feature gates (M0.1, M0.2)

| Package | Publish | Depends on | Must never depend on |
| --- | --- | --- | --- |
| `airs-cli` (default member) | crates.io, lockstep | `airs-runtime-core =V`, `airs-mcp-client =V`, `scm-sdk =0.2.4` | `rusqlite`, `libsqlite3-sys`, `airs-history-store`, `airs-mcp`, any HTTP server framework (the `login` loopback callback is a one-request listener) |
| `airs-runtime-core` | crates.io, lockstep | `scm-sdk =0.2.4`, `jiff`, serde | SQLite, listeners, CLI `Invocation`/output/tenant loading |
| `airs-mcp-client` | crates.io, lockstep | core `=V`, `rmcp =3.8.0` client features, `reqwest =0.13.5` | SQLite, server features, AIRS credentials |
| `airs-history-store` | `publish = false` | core, `rusqlite =0.41.1` | listener, CLI |
| `airs-mcp` | `publish = false` | core, history-store, rmcp server features, axum, jsonwebtoken | the CLI crate |

`V` is the exact shared workspace version. Publication order is **core, client,
CLI**; history-store and server are never published. All four names were unclaimed
on crates.io on 2026-11-04 (read-only API query, nothing reserved). An unavailable
name at release time is a recorded contract revision, not a silent substitute.
Core's SCM dependency must resolve from the registry. M0 proved this with
`cargo package` of the core and client skeletons outside the workspace.

Frozen dependency pins (MSRV 1.89, all type-checked and tested on 1.89.0):

| Crate | Pin | Features | Declared MSRV | Role |
| --- | --- | --- | --- | --- |
| `rmcp` | `=3.8.0` | client: `client`, `transport-streamable-http-client-reqwest`, `reqwest` (+`auth` in the CLI login path); server: `server`, `transport-streamable-http-server` | 1.88 | Official Rust MCP SDK, Streamable HTTP |
| `jsonwebtoken` | `=11.1.0` | `aws_lc_rs`, `use_pem` (no `rust_crypto`) | 1.88 | Two-token verification; reuses the aws-lc-rs provider already linked by rustls |
| `axum` | `=0.8.9` | `http1`, `tokio`, `json` | 1.80 | Server routing only (server crate) |
| `jiff` | `=0.2.39` | `std`, `tzdb-bundle-always` | 1.70 | Embedded, versioned tzdb (no host zoneinfo) |
| `tokio-rustls` | `0.26` (lock 0.26.6) | `aws_lc_rs`, `tls12`, `logging` | — | Backend TLS (server crate) |
| `oauth2` | `=5.0.0` (as resolved by rmcp) | none | — | Only the `TokenResponse` trait rmcp does not re-export |

Feature gates, checked in M0 and re-run in every later phase:

1. The CLI graph never contains `rusqlite`/`libsqlite3-sys` after M1. M1 removed the
   unfinished B-phase code that put them there; `cargo tree -p airs-cli -e normal`
   is checked in each phase's receipts.
2. `serde_json/preserve_order` is never enabled, in either the CLI-only or the unified graph.
3. Exactly one `reqwest` (0.13.5), `scm-sdk` (0.2.4), `serde_json`, `tokio` and
   `rustls` in both graphs. Additive feature changes are allowed only after a full
   parity run with all workspace members included. M0 recorded them: the client adds
   reqwest `json`; the server/test graph adds reqwest `form`, serde_json `alloc`,
   rustls `logging` and tokio `process`.
4. The `airs` binary links the MCP client by default. Without explicit MCP selection
   it executes none of that code (§7.1). M0 measured the release-size delta.
5. **No `tracing` subscriber in the CLI or server may emit rmcp `debug` events.**
   `rmcp 3.8.0` logs the raw authorization code at debug level in
   `exchange_code_for_token_with_issuer`. The server logs only through its own
   allowlisted JSON lines. Any future log integration must filter `rmcp` to `info`
   or above and pass the secret-safety tests.
6. No generic plugin framework, no second SCM SDK, no `rmcp` Tasks/Resources
   dependency for retrieval (§4.1).

## 2. Identity and authorization (M0.3)

### 2.1 Request path and tokens

`Claude Code | airs --mcp NAME` → Goshawks AI gateway (`mcp.goshawks.io/<workspace>/airs/mcp`)
→ `https://mcp-backend-airs.goshawks.io/mcp` (CoreDNS rewrite to
`airs-mcp.airs-mcp.svc.cluster.local`; own namespace `airs-mcp`, PSA enforce
`baseline` for the CHOWN init container) → `airs-mcp`.

Gateway prerequisites (M0.6): the backend host is in the gateway's
`TRUSTED_CUSTOM_HOSTS`; the integration is `oauth_auto` with this service's own
confidential client; a **workspace MCP server** entry and an **AIS access policy**
naming each caller and agent must exist, or the gateway refuses with "denied by access
policy" (policy changes take ~1–2 min to sync). The gateway is its own OAuth
authorization server for clients (`https://mcp.goshawks.io/oauth/<workspace>/<server>`),
issues `pk_…` client IDs to dynamically registered agents such as Claude Code, and
uses Idira public clients as preregistered client IDs. Right after a backend restart
the first call through the gateway can fail; clients retry. The backend terminates TLS, as
Bonbon does. Every HTTP request to `/mcp` carries two tokens, and both are verified
**on every request**: tool calls, list calls, artifact chunks, SSE GET and DELETE.

**Upstream token** (`Authorization: Bearer`, Entra v2 access token):

| Check | Rule |
| --- | --- |
| Signature | Pinned JWKS URL `https://login.microsoftonline.com/<tid>/discovery/v2.0/keys`; `kid` required; algorithms exactly `RS256` (M0.6: confirmed); header `jku`/`x5u`/`jwk` refused. |
| `iss` | Exactly `https://login.microsoftonline.com/<tid>/v2.0`. |
| `aud` | The airs-mcp API **client ID** (Entra issues the app ID even when the resource is the identifier URI; Bonbon ADR 0004) (M0.6: confirmed; a token for another API is `unauthenticated`). |
| `exp`/`nbf` | Required/validated with 60 s leeway. |
| `tid` | Equals the configured Entra tenant. |
| `azp` (v2) or `appid` (v1) | In the configured gateway client allowlist. |
| Delegated only | `scp` present and non-empty; `idtyp=app` or a roles-only token is refused. There is no app-only, static bearer or anonymous route. |
| Permission | `scp` contains one of the frozen permissions in §2.3. |
| `oid` | In the configured integration-owner allowlist. This is an owner constraint, not the caller. |

Entra registration shape (new IDs; never reuse Bonbon's): API app `airs-mcp-api`
with identifier URIs `api://<appId>` and **`https://mcp-backend-airs.goshawks.io/mcp`**
(avoids AADSTS9010010 when the gateway sends RFC 8707 `resource`). Delegated scopes
are under that URI: `.../mcp/AirsHistory.Read`, `.../mcp/AirsHistory.Operate`,
`.../mcp/AirsHistory.Admin`, plus `offline_access` for the gateway's refresh.

**Identity assertion** (`X-User-JWT`, signed by the gateway):

| Check | Rule |
| --- | --- |
| Signature | Pinned gateway JWKS `https://mcp.goshawks.io/.well-known/jwks.json`; `RS256` (M0.6: confirmed). One key signs every integration's assertion. |
| `iss` | Required; exactly `portkey-mcp-gateway` (M0.6 measured). |
| `aud` | Required when listed in `include_claims`. M0.6 measured the gateway's value as the **organisation ID**, identical for every integration in the org. It is enforced as an organisation scope check and is **not** a per-backend binding (§2.2). |
| Lifetime | `iat` and `exp` are required; `exp - iat` ≤ 300 s (the gateway is configured for 120 s). |
| Caller, `client_id` | Required non-empty strings. The caller claim name is configurable: the gateway sends **no `sub`**, and M0.6 uses `email`. `client_id` is the agent: a gateway-issued `pk_…` ID for dynamically registered agents, or the Idira public client ID for the CLI. |
| `workspace_id`, `organisation_id` | Required; each must be in the configured allowlist. The claim names are configurable (§11) because they follow the gateway's spelling. |
| Duplicate headers | Two `Authorization` or two assertion headers are refused (401). |

### 2.2 Per-backend binding finding and decision rule

Source inspection found that the gateway's `user_identity_forwarding` for Bonbon
includes only `sub, client_id, workspace_id, organisation_id, scope`. It is signed by
one gateway-wide key, and Bonbon validates neither `iss` nor `aud`. **No inspected
source establishes a signed per-backend binding.** An assertion minted for any
backend therefore verifies at every other backend under the same key. The required
binding is very likely absent today. M0.6 must determine it from real, sanitized
claim shapes.

The service accepts **only** these resolutions. Each needs a recorded receipt before M1:

1. **Signed audience.** The gateway includes `aud` (or a configured
   integration-binding claim) naming this backend's URL in the signed assertion. The
   server already enforces this (`audiences`, optional `claims.backend_binding`).
2. **Per-integration signing key.** The gateway signs this integration's assertions
   with a dedicated key published at a dedicated JWKS. The server pins that JWKS only,
   so assertions for other backends fail signature verification.
3. **A demonstrated route/token-bound equivalent** with a written replay analysis,
   accepted through a plan revision and independent review.

**M0.6 measured outcome.** Resolutions 1 and 2 are unavailable on this gateway
(Prisma AIRS AI Gateway 2.32.0):

- The signed assertion's `aud` is the **organisation ID**, the same for every
  integration in the org (`iss` is `portkey-mcp-gateway`). It is not a backend binding.
- One gateway key signs every integration's assertion; no per-integration key exists.
- `configurations.custom_headers` is **not** forwarded on `oauth_auto` integrations, so
  a per-route secret header is also unavailable (the first live call was denied for
  the missing header; receipt `step5-first-route-denial.jsonl`).

**Resolution 3: upstream-token binding.** Independent review 8 accepted it, subject
to the conditions in plan revision 7 amendment 1, which independent review 9
accepted. M0 itself is accepted only by a review scoring ≥ 9.1. The per-backend binding is the upstream Entra token, which every request
must carry and which §2.1 verifies on every request:

- `aud` = this API's client ID (`airs-mcp-api`), issued by Entra only for this
  resource;
- `azp` = `airs-mcp-gateway`, a confidential client dedicated to this one integration,
  whose secret is held only by the gateway (and the owner's 0600 provisioning state);
- `tid`, delegated `scp`, `oid` in the owner allowlist; and the grant then binds
  `(caller, client_id)` to that owner `oid`.

The signed assertion still supplies the caller, agent, workspace and organisation, and
its `aud` is enforced as the organisation check.

Replay analysis. An assertion minted for backend B (any other integration on this
gateway) arrives at B with B's upstream token, whose `aud` is B's API and whose `azp`
is B's gateway client. To be admitted here, B, or anyone holding B's traffic, would
need a token with `aud` = this API and `azp` = `airs-mcp-gateway`. Entra issues that
only to the holder of that client's secret, and the gateway sends it only to this
backend over TLS terminated in this pod. An attacker holding only an assertion is
denied `unauthenticated`; receipt 7.3b shows a wrong upstream `aud` is refused even
with a valid assertion. Assertions live ≤ 120 s and upstream tokens ≤ ~1 h.

Remaining trust: the gateway itself (it can mint any assertion and holds this
integration's client secret under every resolution), the confidentiality of the
`airs-mcp-gateway` secret (rotated by `entra/provision.sh --rotate-secret` and
re-entering it in the integration; 90-day expiry), and TLS to this backend. The
difference from resolution 1 is that the binding is carried by a second token rather
than by a signed claim in the assertion; the server refuses any request missing
either one.

Configuration rule: `assertion.audiences` is required and must be non-empty (here the
organisation ID). `[route_binding]` stays in the spike schema only as tested code; it
is unused on this gateway and is not carried into M1.

Short `exp`, `jti` and NetworkPolicy alone are **not** resolutions. If none is
available, **M0 does not pass and M1 does not start**. The fallback is a gateway
change (the user's), not a weaker server check. The spike has no configuration that
accepts an assertion without either a matching signed `aud` or the route secret, and
none that skips the upstream checks. A mismatched `aud` is denied
(`forbidden`, "identity assertion is not bound to this backend").

Spike diagnostics (M0.6 only; M2 decides its own audit fields per §2.5). Every
`allowed`/`denied` line carries claim **shapes** (allowlisted names → JSON types).
After signature verification, a denial may also carry these public identifier
**values**, each bounded to a string of ≤128 characters or replaced by its kind:
`assertion_registered` = `{iss, aud, workspace, organization}`, and `ungranted` =
`{sub, client_id}` (the configured caller claim, which on this gateway is an email
address). A session conflict logs `differs`, the names of the differing tuple fields
only. Tokens, signatures, secrets and other claim values are never logged.

### 2.3 Principal, permissions and grants

Principal tuple: `(tid, upstream oid, assertion caller, assertion client_id, workspace,
organisation, service tenant)`. "Caller" is the configured caller claim (`sub` by
default; `email` on this gateway, which sends no `sub`), and `sub` below means that
value. Missing elements deny. Grants, idempotency scope and
audit actor are keyed by **`(sub, client_id)` within the service tenant**. `oid` must
equal the grant's configured `owner_object_id`. Different identifier kinds are never
equated. Multiple `(sub, client_id)` grants are supported. An unknown agent or caller
has no access (403 `forbidden`).

Frozen permission identifiers (Entra delegated scope names, and server tiers):
`AirsHistory.Read` → Reader, `AirsHistory.Operate` → Operator,
`AirsHistory.Admin` → Administrator. Admin ⊇ Operator ⊇ Reader. **Effective
authority = token scope tier ∩ grant tier ∩ grant allowed-tool list.** Each call is
reauthorized against current configuration. Tool annotations are hints only, and
`tools/list` shows only callable tools but is not authorization.

**M1 is stateless at the MCP layer.** The server issues no `Mcp-Session-Id`
(streamable HTTP stateless mode). Every request is authenticated and authorized on its
own tokens, and nothing about the caller is kept between requests. Paging and
continuation state lives in the signed cursors of §4.2, whose scope digest includes
the principal, so a cursor never crosses principals. Jobs, receipts and idempotency
keys are durable server state keyed by `(sub, client_id)`, never by a session.

Why (M0.6 step 7.6): the gateway forwards **one upstream session per (user, server)**
to the backend, whichever agent is calling. With the spike's principal-bound sessions,
Claude Code (Reader) arrived bearing the session the admin CLI had initialized: 15 of
15 requests authenticated and were then refused "session belongs to another
principal" (`differs` = `client_id` only). The gateway kept attaching that session on
reconnect, locking the agent out until the spike restarted. Binding refused every
cross-agent use, but concurrent agents of one user cannot work with sessions.

If a later revision needs sessions, it must keep the spike's rule (a session is bound
at creation to `(tid, oid, caller, client_id, workspace, organisation)`; any other
principal gets 403; capacity counts bound sessions plus reservations under one lock),
**and** give sessions an idle expiry and answer an expired or unknown session with
404 so clients reinitialize. That is a contract revision, not an M1 option.

Default grants (configuration examples; the user supplies the real tuples):

| Client | Tier | Default allowed tools |
| --- | --- | --- |
| Claude Code agent `client_id` | Reader | all Reader tools (§4.3) except `artifacts_download` (CLI byte transfer; keeps base64 out of agent context) |
| Agent with an explicit Operator grant | Operator | `collection_start`, `reports_create`, `jobs_cancel`, `receipts_ingest`, `assessments_submit` — **excluding** `schedules_set`, `schedules_disable` and `enrichment_start` unless each is listed explicitly |
| Optional CLI client, ordinary | Operator (non-admin connection) | as configured; used for `--record-history` |
| Administrative CLI client | Administrator | effective only under §3 |

### 2.4 Discovery, metadata and SSRF rules

- `GET /.well-known/oauth-protected-resource` and `.../oauth-protected-resource/mcp`
  (RFC 9728) are unauthenticated. They return `resource`, `authorization_servers`
  (the Entra v2 issuer), `scopes_supported` and `bearer_methods_supported: ["header"]`.
  They expose no tenant data. 401/403 responses carry
  `WWW-Authenticate: Bearer resource_metadata="<origin>/.well-known/oauth-protected-resource/mcp"`.
- JWKS: HTTPS only (loopback HTTP only in the test configuration, refused off
  loopback), no redirects, no proxy, 5 s timeout, 64 KiB body cap, 10 min cache,
  and at most one refetch per 60 s for an unknown `kid`. A failed refresh keeps the
  previous set. Key origins come only from configuration, never from a token, tool
  input or discovery response.
- The CLI's OAuth discovery uses rmcp's resolver. Discovered hosts may be loopback
  only when the base URL is loopback, link-local/reserved addresses are rejected, and
  cross-origin redirects are rejected. Configured approved discovery origins (§7.2)
  are checked in addition.

### 2.5 Audit and revocation

Audit records hold only `(sub, client_id)`, the grant label, the job/operation ID,
the approved scope digest, the policy revision, the time and the outcome. Never
tokens, claim values beyond those identifiers, raw bodies, headers or URL queries.
Accepted jobs survive token expiry. Before each resumed phase, outbound batch and
destructive commit, the worker rechecks the *current* grant of the job's
submitter, or of the service actor for schedules. Revocation becomes visible on the
next configuration reload or token validation. The service claims no instantaneous
directory revocation.

## 3. Administrative authority (frozen)

Configuration `admin.mode` is either `disabled` (the default) or `fresh_proof`.

1. **`disabled`:** no administrative mutation is admitted from any client. Read-only
   preview tools stay available to Administrator grants. This is the required
   setting unless an option below is qualified in M0.6. **An administrative identity
   with effective mutating grants is provisioned only under a qualified option.**
   Without one, admin mutations run only from an isolated context (option A) or stay
   disabled.
2. **Option A, isolated context:** the administrative OAuth session exists only on an
   account or device outside every agent's privileges: shell, credential stores and
   browser profile. 0600 files under the agent's own user do not qualify. It must pass
   the same shell-context negative probes as option B.
3. **Option B, `fresh_proof`:** every mutation under the administrative identity
   requires a gateway-signed assertion carrying (a) the server-issued nonce of the
   exact preview, in claim `admin.proof_nonce_claim`, and (b) `auth_time` no older
   than `admin.proof_max_age_seconds`. This covers direct execute, approval issuance,
   maintenance repair, allowance policy and lower-tier mutations reached through that
   identity. A recent token, bare `auth_time`, pty or typed phrase never qualifies.
   No admin access or refresh token able to authorize a second mutation may persist
   in agent-readable storage. The CLI's admin connection therefore keeps no reusable
   token cache: the M7 admin connection is `cache = "none"`. Option B qualifies only
   if M0.6 proves the gateway can sign and forward such a nonce-bound proof. Source
   inspection found **no `auth_time`/`max_age`/step-up support** in Bonbon or the
   gateway configuration, so this option is expected not to qualify initially.
4. **Authority classes are frozen per tool** in `schemas/tools.v1.json`
   (`authority` + `authority_rules`): `grant`, `direct_proof`,
   `direct_proof_or_approval` (`lifecycle_execute`) and `upload_session`
   (`transfer_upload_chunk`/`_finalize`/`_abort`). Uploads are admitted only
   through a `lifecycle_preview` with `action = "upload_admit"`. That preview binds
   the kind, total length, SHA-256 and expected TSG/environment. `transfer_upload_begin`
   consumes it with direct proof and creates a session owned by that actor.
   Chunks continue that one admitted operation without new proof: only the owner,
   contiguous offsets only, never beyond the declared length, and within the 24 h
   expiry. Abort is by the owner only. Importing the finished upload is a separate
   `import_finalize` preview that needs its own authority. The executable model
   `crates/airs-runtime-core/src/authority.rs` encodes every transition below, plus the
   upload rules. Its tests cover the valid direct, approval and upload flows, and
   deny: cross-operation proof reuse, foreign actors, stale/missing proof, digest and
   generation changes, expiry, approval reuse or transfer, self-approval, wrong-action
   upload admission, foreign chunks/aborts, replayed offsets and oversize uploads.
   M3 implements each model transition as one control-store transaction.
5. **Mutation surface of an administrative identity** is defined by authority class,
   not by the `admin_proof` flag. An Administrator grant may call:
   - read-only tools;
   - `direct_proof` tools (`admin_confirm`, `transfer_upload_begin`);
   - `direct_proof_or_approval` tools (`lifecycle_execute`);
   - `upload_session` tools (`transfer_upload_chunk`, `_finalize`, `_abort`), each
     under its class's rules.

   `upload_session` tools carry `admin_proof: false` because they need no new proof:
   each is a continuation of an upload already admitted by direct proof. Any mutating
   tool of class `grant`, including Operator tools such as `collection_start` or
   `jobs_cancel`, is refused for that identity in **every** mode. This closes the
   lower-tier side door (R1a). The spike enforces the rule centrally before dispatch
   and tests it with cached admin sessions in both modes.
6. **Idempotent retries must repeat the exact admitted binding.** For
   `lifecycle_execute` the binding is the preview ID, parameter digest and data
   generation, plus the approval ID on the approval path. For `transfer_upload_begin`
   it is the preview ID, digest and generation. A retry with the same key returns
   the original job or upload ID only when every bound field is identical. Any
   difference is `idempotency_conflict`, never the original result and never a new
   job.
7. **Upload lifecycle:**
   - `upload_admit` previews are consumed only by `transfer_upload_begin`. They are
     refused on `lifecycle_execute` and `admin_confirm`.
   - `transfer_upload_finalize` succeeds only for the owner, with the complete
     declared length and a matching SHA-256. It is idempotent once finalized.
   - `import_finalize` previews can be created only by the upload's owner, only for
     an unexpired finalized upload. The *executing* transition rechecks ownership,
     expiry and the finalized/unconsumed state, and consumes the upload together
     with the job insert. Overlapping previews therefore admit exactly one import,
     and a later abort or expiry blocks any outstanding preview. Each import needs
     its own direct proof; the upload's proof never carries over.
   - Abort is refused after import.

Frozen proof → receipt → job transitions (control store; manifest in maintenance):

| From | Event | To | Effect |
| --- | --- | --- | --- |
| — | `lifecycle_preview` (Admin grant) | `preview_open` | Persist the preview ID, tenant, action, parameter digest, data generation, actor `(sub, client_id)`, nonce, expiry (300 s). No mutation. |
| `preview_open` | `lifecycle_execute`, valid proof, same actor/digest/generation, unexpired | `consumed` + job `queued` | Mark the preview consumed **and** insert the job **in one control transaction**. Return the job ID. |
| `consumed` | retry of the same execute (same idempotency key) | `consumed` | Return the same job ID. No second job. |
| `preview_open` | missing/stale/foreign proof, or changed parameters | `preview_open` | Deny (`forbidden`/`conflict`), unchanged. |
| `preview_open` | expiry, or a data-generation change | `expired` | A new preview is required. |
| job running | restart/checkpoint | job running | Resume without new proof. Revocation and grant rechecks still apply. |
| `preview_open` (agent actor) | `admin_confirm(preview_id, digest)` by the admin identity with valid proof | `approved` + approval `issued` | In one control txn: bind approval `q1_…` to the preview ID, digest, data generation, agent actor, and an expiry ≤ the preview expiry. The proof nonce is consumed. |
| `approved` | `lifecycle_execute(preview_id, digest, approval_id)` by the **same agent actor** | `consumed` + approval `consumed` + job `queued` | One control txn. Any mismatch (actor, digest, generation, expiry, approval) is denied with no state change. |
| `approved` | expiry or data-generation change | `expired` | The approval becomes unusable. |

An agent-prepared action uses `admin_confirm(preview_id)` instead, which issues a
one-use approval receipt bound to the same tuple. It is subject to the same option-B
proof and the same single-consumption rule. In maintenance mode, agent-prepared
execution is refused entirely.

## 4. Tool protocol (M0.1)

### 4.1 Transport and versions

Streamable HTTP at `/mcp`. The server supports MCP protocol versions `2025-06-18`,
`2025-11-25` and `2026-07-28` (rmcp 3.8.0). It records the negotiated
`MCP-Protocol-Version` per request. Claude Code's and the gateway's actual version
is `2025-11-25` (M0.6 receipt). Durable jobs are ordinary tools, and retrieval never
depends on MCP Tasks or Resources. The transport is stateless (§2.3): no MCP sessions,
one replica and `Recreate`. A restart loses no client state; jobs are unaffected.

Stateless request handling (spike `sessions = "stateless"`, rmcp
`legacy_session_mode = false`, `json_response = true`). Review 9 inspected the
pinned rmcp source to confirm it; the live rows are 7.6 in the receipts.

- **Initialization:** `initialize` succeeds and negotiates the version, but creates no
  server state. No `Mcp-Session-Id` is ever issued.
- **Ordinary requests:** each POST is authenticated and authorized on its own tokens
  and answered with one JSON response, with no SSE stream. A missing
  `MCP-Protocol-Version` header falls back to rmcp's compatibility default, and an
  unsupported version is rejected. Stateless mode keeps no negotiated version between
  requests, so an ordinary request is checked against the supported set only, never
  against an earlier `initialize` (review 10 probe: initialized with `2025-06-18`, then
  called with `2025-11-25`, accepted). The only mismatch check compares an `initialize`
  request's header with that same request's body.
- **Forwarded or stale `Mcp-Session-Id`:** ignored. It never selects state and never
  identifies a principal; the spike logs `session_header_ignored`, without the value.
- **GET and DELETE:** answered with 405 after authentication. There is no standalone
  SSE stream and no session to delete.
- **Retry:** an MCP-level failure from the gateway (for example the 502 after a
  restart) is retryable for read-only tools. A mutating tool is retried only with the
  same `idempotency_key` (§3 rule 6), never as a new call.

### 4.2 Envelope

Every tool input has the shape
`{"schema_version": 1, "tenant": "<public service tenant id>", "idempotency_key"?: "...", "expected_epoch"?: "<8 hex>", "params": {...}}`.
`idempotency_key` and `expected_epoch` are both required on idempotent tools. The
epoch is the value `service_info` reported when the client created the key, and the
CLI stores it in its operation receipt. If the server's current recovery epoch
differs, the call returns `control_restored` and no job is created, so a key minted
before a control-store restore can never silently resubmit.
`deny_unknown_fields` applies at every level. A different `schema_version` is
`unsupported_version`. Validation completes before any effect. There is no SQL,
filesystem path, URL, template or shell input anywhere.

Results: `structuredContent` `{schema_version, ...}`. Errors: `isError: true` with
`{schema_version, code, reason, retry_after_seconds?, job_id?, credential_status?}`.
`reason` comes from a compiled vocabulary and never carries SDK, SQLite or HTTP text.

| Code | Meaning | CLI exit |
| --- | --- | --- |
| `invalid_request`, `unsupported_version` | Server-side validation (the CLI validates usage first; its own usage errors exit 2) | 1 |
| `unauthenticated` (HTTP 401) | Token missing/invalid/expired | 1, "authentication required: run `airs mcp login`" |
| `forbidden` (403) | Grant/tool/tier/binding denial | 1 |
| `result_too_large` | A result would exceed its response bound (§5); the spike used `internal` here (M0.6 finding) | 1 |
| `not_found`, `conflict`, `idempotency_conflict`, `control_restored` | Identity/state conflicts | 1 |
| `overloaded`, `quota_exhausted`, `low_space` | Bounded admission, with `retry_after_seconds` | 1 |
| `storage_unavailable`, `maintenance_required` | Ordinary operations unavailable | 1 |
| `credential_blocked`, `upstream_unavailable` | AIRS credential/plane state | 1 |
| `canceled` | Job canceled | 1 |
| `partial` (a result status, not an error) | Partial report/collection | 0, or 1 under `--strict` |

Cursors are opaque `c1.` + base64url(JSON `{scope_digest, snapshot, position}`) +
HMAC-SHA256 under a per-epoch server key held in the control store. A cursor from
another scope, tenant, principal or epoch is `invalid_request`. A page holds 1–100
items (default 50).

Idempotency keys: 16–64 characters `[A-Za-z0-9_-]`. The CLI generates a UUIDv4. The
scope is `(service tenant, sub, client_id, operation, recovery epoch)`. The same key
with the same canonical parameter digest returns the same job; a different digest
is `idempotency_conflict`; a key from another epoch is `control_restored`. Job IDs
are `j1_<epoch8hex>_<uuidv4>`; preview IDs are `p1_<epoch8hex>_<uuidv4>`. Every
server-issued identifier suffix is a canonical lowercase UUIDv4 (version nibble 4,
variant 8/9/a/b), checked identically by the schema and the compiled decoder. AIRS
scan IDs are any canonical lowercase UUID.

### 4.3 Frozen tool catalogue

| Tool | Tier | Idempotency key | Purpose |
| --- | --- | --- | --- |
| `service_info` | Reader | — | Version, contract/schema versions, negotiated protocol, effective tier/tools, health summary. No tenant data beyond the public tenant ID. |
| `history_status` | Reader | — | Archive presence, schema, last collection, falling-behind/gap summary. |
| `history_coverage` | Reader | — | Per-source traversed-as-of coverage for a ≤90-day half-open UTC range. |
| `history_query` | Reader | — | Bounded aggregate/cohort summaries (paginated); never raw payloads. |
| `reports_list` / `reports_get` | Reader | — | Report/version listing (paginated) and manifest/summary. |
| `artifacts_read` | Reader | — | Agent-facing UTF-8 pages of `review.md` or summaries, ≤16 KiB each. |
| `artifacts_download` | Reader | — | CLI byte chunks (§5). Every chunk is reauthorized and hash-bound. |
| `jobs_list` / `jobs_get` | Reader | — | Job state, checkpoints, results, gaps. |
| `schedules_list` | Reader | — | Schedules, last completed slots, backlog. |
| `collection_start` | Operator | required | Collection/backfill submission (§6). |
| `reports_create` | Operator | required | Manual report (quota §6). |
| `enrichment_start` | Operator, explicit | required | Strategic examples (Reports quota). |
| `jobs_cancel` | Operator | — (idempotent by job) | Explicit cancellation. |
| `schedules_set` / `schedules_disable` | Operator, explicit | required | Schedule management with effective times. |
| `receipts_ingest` | Operator | required (batch key) | ≤200 records/256 KiB normalized `client_asserted` receipts. |
| `assessments_submit` | Operator, explicit for agents | required | Attributed agent assessment (never overwrites evidence). |
| `lifecycle_preview` | Administrator (read-only) | — | Prune, artifact delete, compact, backup, restore, recover, import finalize, export, allowance set, epoch activate. |
| `lifecycle_execute` | Administrator + §3 | required | Executes exactly one preview. |
| `admin_confirm` | Administrator + §3 | — | One-use approval receipt for an agent-prepared preview. |
| `transfer_upload_begin` / `_chunk` / `_finalize` / `_abort` | Administrator | begin: required | Staged upload (§5.4). |
| `export_read` | Administrator | — | Chunks of a completed whole-archive export. |

The exact versioned parameter and result schemas are frozen in
`crates/airs-runtime-core/schemas/tools.v1.json` (moved from the M0 prototype in M1). That file
also holds the shared `$defs`: identifiers, ranges, cursors, jobs, manifests,
receipts and the error envelope. It records each tool's tier, idempotency,
`mutating`, `admin_proof` and result-limit class. `examples.v1.json` holds one valid
request and result per tool, closed job-result variants (collection, report,
enrichment, lifecycle, export, failure) with succeeded/partial/failed job examples,
and the `upload_admit` preview request. Both are generated by `schemas/generate.py`. The core
test `tests/schemas.rs` checks the catalogue, tiers and the admin-proof rule; checks
that every object is closed and every `$ref` resolves; validates every example; and
checks that for every tool an unknown top-level or parameter field, `schema_version`
2, an invalid tenant, a missing idempotency key or a missing epoch is refused. M1
moves these files into the production core crate unchanged. A schema change after M0
is a contract revision.

**Service implementation status (M2).** The `airs-mcp` server lists and dispatches from
this catalogue (it never restates tiers or authority classes). M2 implements
`service_info`, `history_status` and `history_coverage`. Every other catalogue tool is
checked against grants first (`not_found` for an unknown name, `forbidden` for a tier,
grant or administrative-side-door denial), and is then refused as `invalid_request`
("tool not available in this service version"). It is never listed or reported in
`service_info.tools`. The dispatch order is: catalogue, grant and tier, administrative
side door, implemented, maintenance (`maintenance_required` for every tool but
`service_info`), strict envelope (`invalid_request`, `unsupported_version`), served
tenant (`forbidden` unless `tenant` equals `[service] tenant`), the tool, and the
result bound (`result_too_large`). Each call writes one audit line with the §2.5
fields: `time`, `sub`, `client_id`, `grant` (label), `tool`, `job_id`, `scope_digest`
(SHA-256 of the tool, tenant and canonical arguments), `revision` and `outcome`.
Authentication logs carry only fixed codes, the permission tier, the revision and, for
an ungranted but verified caller, its `(sub, client_id)`; never claim values or
shapes. Transport refusals from the MCP library are answered with a fixed body per
HTTP status, and JSON-RPC error messages are replaced with fixed text, so nothing a
client sent is reflected.

- `history_status` (from M4, derived from durable records): `archive_present` and
  `schema` come from the archive; `last_collection` is the view of the tenant's most
  recently submitted `collection_start` job (`null` before any); `falling_behind` is
  true when the collection schedule in effect has pending (backlogged) slots or a
  windowed or other polled coverage group of the last 24 h is `falling_behind`;
  `oldest_gap_seconds` is the largest polled coverage gap of the last 24 h (`null`
  without an archive).
- `history_coverage` reports one item per source category and scope
  (`application_id` `null` = tenant-wide), sorted by source then application.
  Aggregation is conservative (review M2-1).
  - **Scope is kept.** A tenant-wide traversal (`scope_kind` `cloud`, supported) counts
    for the tenant-wide item only. An application or selection traversal counts for each
    named application only, never tenant-wide. A segment with an unsupported or unknown
    scope is never complete and covers nothing.
  - **Sources are not merged.** Completeness is decided per underlying source within a
    scope: `traversed_as_of` is complete; legacy `observed` and `partial` are partial;
    `unavailable` and `falling_behind` cover nothing; any uncovered time makes a source
    at least partial. A category is complete only if every underlying source recorded
    for that scope is complete and gapless. Its state is the worst source state and its
    `gap_seconds` the largest source gap (rounded up), so disjoint halves from two
    sources never add up to completeness. `traversed_as_of` is the earliest covering
    traversal.
  - Source categories:
    - `sessions` covers `sessions` and `session-details`;
    - `results` covers `scan-results`;
    - `reports` covers `scan-reports`;
    - `inventory` covers `applications`, `registered-apps`, `application-telemetry` and
      `transaction-metadata`;
    - `profiles` covers `profiles`.
  - Local evidence-handling sources (`event-window`, `evidence-storage`,
    `archive-range`) and unrecognized legacy names are not reported.
  - An `offline-gap` marker makes every category of its recorded scope `falling_behind`;
    it is not a source and adds no gap of its own.
  - A category with no segment is absent, meaning unknown. A missing archive is
    `not_found`.
  - At most 100,000 segments and 10,000 groups are aggregated per request; beyond that
    the request is refused rather than truncated.
- Cursors: `c1.` + base64url(JSON `{d, s, p}` ‖ HMAC-SHA256 tag). Here `d` is the scope
  digest of the tool, tenant, `sub`, `client_id`, recovery epoch and every non-page
  parameter; `s` is the digest of the complete result set; and `p` is the position. The
  per-epoch key lives with the epoch (§9.3). A cursor from another scope, or with a bad
  tag, is `invalid_request`; a changed result set is `conflict` (restart without a
  cursor). Each page is the largest within `limit` whose complete result fits 16 KiB.

## 5. Response and transfer bounds (M0.1, measured in M0.2)

Bounds apply to the **complete serialized `CallToolResult`** a client receives, not
just the structured value. rmcp's `CallToolResult::structured` duplicates the value
as text content, which roughly doubles the size. The JSON-RPC envelope adds under
200 bytes on top.

1. **Agent-facing results** (`service_info`, `artifacts_read`, `reports_get`,
   `history_*`, previews): ≤16 KiB complete. The server sizes each page adaptively
   until the complete result fits, and refuses rather than truncates. In the
   loopback prototype the largest complete page is **15,672 bytes**. No HTML/base64
   payload enters an agent's context, and there are no public URLs.
2. **CLI byte chunk** (`artifacts_download`, `export_read`): 64 KiB raw, base64.
   The complete result measured **175,406 bytes**, within the ≤256 KiB ceiling. The
   CLI also enforces 256 KiB client-side and verifies every chunk hash. A 32 MiB
   artifact took **512 calls**, about 9–11 s in a loopback debug build, with SHA-256
   verified end to end. Gateway and Claude Code limits and elapsed times are
   **(M0.6)**. If the gateway refuses ~175 KiB results, the chunk drops to 32 KiB
   (~88 KiB complete) or 16 KiB (~44 KiB complete). Never public URLs.
3. **Request bodies**: ≤256 KiB (rmcp `max_request_body_bytes`); larger is 413.
4. **Uploads**: 64 KiB raw per chunk. Per-tenant staging quota is 1 GiB by default
   (configurable up to the 16 GiB transfer ceiling). An upload expires 24 h after it
   begins; expired (abandoned) uploads are aborted and their staging removed by the
   staging sweep, which runs on every upload begin and on every worker pass. Large old-archive migrations use the server-side read-only staging
   path (plan §History transfer), not hundreds of thousands of MCP calls.
5. Downloads stream to a private temporary file under the destination directory
   (`O_EXCL`, 0600). Every chunk's offset and hash are checked, then the total length
   and SHA-256 against the manifest. The file is published without clobbering (hard
   link + unlink) and the directory fsynced. A failure removes only the CLI's own
   temporary file. The loopback test also proves a second download cannot overwrite.

### 5.1 Implementation status (M5: transfer and lifecycle)

- **Logical import** (`airs_history_store::transfer::import_file_controlled`). Input is
  data only, never SQL, schema or pages. Every content check runs on a private staging
  copy before the destination is touched:
  - framing, per-table counts, the exact-byte SHA-256 footer and no trailing data;
  - the header's version, schema digest and explicit cloud binding;
  - every row's cardinality, types and safety allowlist (including the strict shapes of
    service run provenance: the bound credential revision, write-time fact counts and a
    receipt run's first-acceptance record), and insertion into a staging store of this
    schema, so duplicate keys refuse;
  - whole-set referential integrity, so orphans refuse;
  - semantics: every event's identity is the identity its facts name; every example is
    a row its event's facts derive (so an unowned report or a forged excerpt refuses);
    every work descriptor decodes into its strict type and names its work; every
    receipt's ID is its origin key's digest (never empty) and at least one fact of its own
    supports it; and the events, their reports, the detector observations and the
    receipts are exactly what the archive's projection derives from the staged facts,
    visited in keyset pages that start before every key (the empty string included). A
    difference refuses; nothing is silently rebuilt.

  Staging and semantic validation are disk-backed: a private staging database with a
  bounded page cache and on-disk temporary storage, derived identities and examples in
  staging tables, and projections compared in SQL (`EXCEPT` both ways) after a
  page-by-page rebuild, under the import's deadline and cancellation, so memory does not
  grow with the transfer.

  Before the first batch, the whole staged set is checked against one destination
  snapshot (a write transaction under the writer gate, rolled back): the preview's
  generation binding, every held key (a key the archive already holds must carry
  identical immutable content; its first observation time and run are the
  destination's own metadata; projection rows are rebuilt) and every erasure record (no
  example, event fact or promoted receipt fact may carry an excerpt the destination
  pruned). A conflict anywhere in the transfer therefore refuses before any destination
  change. A destination that did not exist must still be empty when the first batch
  creates it (another writer creating it with data in between refuses).

  The destination is then changed in bounded batches (1,000 rows, positioned within
  their table rather than rescanning earlier rows), each in its own write transaction
  under the archive's exclusive writer. The first batch must find the generation the
  whole-transfer check saw, and every later batch exactly the generation the previous
  batch left, so any interleaved write stops the import. The whole-transfer check of a
  version-1 (accepted-M4) destination never upgrades it, so a refusal leaves it byte for
  byte unchanged; the first batch's writer upgrades it (below), and that upgrade, the
  writer's own change, is the only generation change a binding to the version-1
  generation still admits. Every batch also writes its
  progress (operation ID = the job ID, transfer SHA-256, batches, rows, the generation it
  started from) to the archive's own `import_operations` journal in the same
  transaction, and the executor then records the control checkpoint (`importing`). On a
  restart the archive journal, not the control checkpoint, decides where the import
  resumes, so a crash between a batch commit and its checkpoint is reconciled (C4); a
  refusal after committed batches reports them (`partially_committed`), never
  `refused_before_change`. A final transaction rebuilds every touched projection. The
  result distinguishes `refused_before_change`, `partially_committed` (with the committed
  batches and rows) and `uncertain` (a commit whose outcome is unknown). A cancellation
  observed at a batch checkpoint stops before the next batch or the rebuild; the job ends
  `canceled` with the committed batches reported; a cancellation that arrives after the
  last checkpoint keeps the executor's committed result (success, partial or failure) on
  the canceled job. Every batch also rebuilds, in its own transaction, the projections
  of the events and receipts it added facts to, so every committed batch leaves
  projections consistent with every fact the archive holds. Until the final transaction
  publishes the import, the archive's `import_operations` row records it as unfinished:
  the history status reports `unfinished_imports`, and `history_coverage` refuses
  `maintenance_required` rather than present a partially imported archive.
- **Archive generation** (`inspect::data_generation`): the archive's incarnation
  (`archive_generation`) in the high 32 bits and SQLite's file change counter in the low
  32. The counter advances with every committed archive write (a fact linked inside an
  existing run, an update, a prune). The incarnation is a durable monotonic counter:
  compaction (whose `VACUUM INTO` copy restarts the file counter) and recovery advance
  it before the file is published, so within an archive's lineage a generation never
  repeats. A new lineage (a created archive, or an upgraded version-1 archive) starts at
  a random value below 2^30, so an archive recreated at the same path is unlikely (a
  2^-30 chance), not guaranteed, to start where a deleted one stood; exhaustion at
  2^31 - 1 is refused, never wrapped.
  Previews are bound to it; a prune preview's counts and its generation come from one
  read transaction. Destructive execution checks the binding inside its own write
  transaction (prune, import); `compact` checks it immediately before running.
  `recover` is bound instead to the archive's and rollback journal's file identity
  (device, inode, length, modification time and headers), read without admitting
  either to SQLite: an archive that needs recovery has a journal, which the ordinary
  reader refuses. A keyed retry is resolved against its original admitted binding, so a
  retry after the admitted operation itself changed the archive returns the original
  job or upload.
- **Schema versions.** The archive schema is version 2. A version-1 archive (exactly the
  accepted M4 schema) is read as is. The first writer that may change it upgrades it in
  place, under the writer gate, only after a verified private copy of the version-1
  file (`.pre-upgrade-v1.<id>.sqlite3`, its size and SHA-256 in the durable
  `.pre-upgrade-v1.<id>.json` manifest) exists: one exclusive transaction adds
  `pruned_examples`, `archive_generation` and `import_operations`. A destination check
  that must not change the archive opens it without upgrading. A hot journal on a
  version-1 archive is recovered like any other (verified copy first, raw copies kept),
  and the recovered archive becomes version 2 in the same verified transaction. An
  unknown or newer schema is refused unchanged.
- **Examples-only prune** (`repository::prune_examples_bound`):
  - The preview counts and writes nothing. The selection is SQL (a subquery for the
    preview, a temporary selection table under the writer), never one bound list of
    IDs, and every fact copy is visited in bounded keyset pages, so an ordinary large
    selection (tens of thousands of events) stays within SQLite's 1 MiB value limit and
    is never materialised in memory. The result reports the tombstones actually
    recorded.
  - Execution records a `pruned_examples` tombstone for every excerpt any copy carries
    (the `examples` rows, and every snippet of every event fact and promoted receipt
    fact, including snippets no `examples` row holds), then removes every live excerpt
    copy, rebuilds the projections (verdict, action and metadata stay), and compacts the
    file.
  - Tombstones are enforced on every later write path: a repeated fact or a new variant
    of it is stored with the pruned excerpts erased (keeping the received fact's ID), a
    pruned example is never recreated, a receipt promoted to the event has them erased
    from its facts, and an import carrying them in any fact copy is refused.
  - Earlier exports and external backups are not erased; there is no forensic claim.
  - Tombstones are local erasure records and are not transferred.
- **Service tools:**
  - `lifecycle_preview` (Administrator, read-only) reports the affected data and a
    single-use preview bound to the canonical parameters, their digest and the archive
    generation.
  - `lifecycle_execute` (direct proof or a one-use approval) and `admin_confirm` (direct
    proof, never one's own preview) use the M3 authority store. The proof is the verified
    assertion's configured nonce claim and its `auth_time`. Administrative execution
    requires `[admin] mode = "fresh_proof"`.
  - `transfer_upload_begin` (direct proof) consumes an `upload_admit` preview and creates
    an owned private staging file of the declared length and hash, reserving the declared
    length against the tenant's aggregate staging quota (`[limits]
    staging_quota_bytes`; `overloaded` when the live uploads already reserve it). A
    replayed begin returns the upload's persisted expiry. Every upload whose bytes may
    still be staged reserves them: open uploads until they expire, consumed (`imported`)
    and aborted uploads until their staging is verifiably removed (`removed`). The
    staging sweep (on every upload begin and every worker pass) aborts expired uploads
    and removes the staging of aborted uploads and of uploads whose consuming job has
    ended, whatever its outcome; it releases the quota only after the removal, and a
    restart reconciles whatever an earlier sweep left.
  - `transfer_upload_chunk` checks each chunk's hash, then ownership, contiguity and the
    declared bound. The transitions of one upload are serialized; the bytes are written
    and synced before the progress is recorded, so a failed write or a crash leaves the
    progress unchanged and the chunk can be sent again; a retry of a recorded chunk is
    accepted only when the staged bytes at its offset are exactly its bytes
    (`idempotency_conflict` otherwise).
  - `transfer_upload_finalize` checks the owner and the complete length before reading
    any content, then hashes the staged bytes by streaming and validates them against the
    whole admission declaration: length, SHA-256, `expect_tsg` and `expect_environment`
    (this service's cloud) and the content for the declared `kind`, with the service's
    complete secret registry. Only a valid upload is published `finalized` (importable);
    an invalid one is aborted and its staging removed. Validations run one at a time
    (`overloaded` otherwise).
  - `transfer_upload_abort` (owner, also after expiry) removes the staging file; staging
    is removed only when it is the service's own regular file.
  - `export_read` serves hash-bound chunks of a finished export, bound to that file's
    SHA-256 (streamed when the export is published).
  - Export, upload validation and import apply the service's complete secret registry
    (every configured credential file), refusing `credential_blocked` when any cannot be
    loaded.
- **The `lifecycle_execute` executor** runs these actions online:
  - examples-only `prune`;
  - `compact` and `recover`;
  - `export` (a private logical transfer under the data subtree);
  - `import_finalize`, by the upload's declared kind: `logical_transfer` (the full staged
    validation, then the bounded checkpointed import above) or `legacy_bundle` (a legacy
    evidence bundle through the operator-bound legacy import; validated at finalize).

  **Scope reconciliation (amends the M5 plan row).** `backup`, `restore` and
  `epoch_activate` stay offline maintenance of the stopped service (`airs-mcp control-*`,
  which backs up and restores the *control store*; the history archive's own
  backup/restore is the operator's volume procedure, M8). `artifact_delete` addresses
  report artifacts and is delivered with M6 (§6.2). `allowance_set` and non-examples prune scopes are not offered
  online in this version. Each of these is a fixed `result_failure`, never a partial
  effect. M5.4 is therefore `compact` and `recover` through service tools; restore is
  offline.
- **Old-archive migration** (`airs-mcp migrate-archive --config PATH --source DIR`,
  offline maintenance of the stopped service): the recognized archive of this
  service's cloud under `DIR/<cloud_id>/archive.sqlite3` (a regular single owned file
  without journal/WAL sidecars, at most 16 GiB) is copied into private staging and
  verified, exported as a logical transfer by the store's own admission (a version-1
  accepted-M4 archive is recognized and upgraded on the private copy; an unknown or newer
  schema, or another cloud, refuses), a verified rollback copy of the destination
  is kept when one exists, and the transfer is imported through the full validation. The
  original is opened read-only and verified unchanged afterwards.

## 6. Limits, budgets and quotas (frozen defaults)

| Item | Default | Bound | Notes |
| --- | --- | --- | --- |
| Collection interval | 15 min, UTC quarter-hours | 5–60 min | Missed ticks coalesce into one catch-up job. |
| Tick lower bound | last traversed-as-of − 15 min overlap | narrowest rolling selector, ≥1 h | Advances per source only after a valid traversal. |
| Tick budget | 120 s, 200 sends incl. OAuth, 40 pages/source | config ≤ 600 s / 1000 / 200 | A ceiling, not a per-application allowance. |
| First collection | recent 24 h | — | Older backfill explicit, ≤87,600 h selector bound. |
| Reports per job | 5 IDs default, 10 max, ≤2 sends | compiled | Serial, stop on 429, cached reuse. |
| Reports per tenant/Chicago day | 2 sends, 10 IDs | Admin ≤10 sends / 50 IDs | Charged to both job and admission day; high-water guard (§8). |
| Manual reports | 4 per principal per rolling hour; 24 per tenant/day | tenant ceiling 48/day | Charged at admission; cancel never refunds. |
| Queue | 64 queued jobs/tenant, 8/principal | — | `overloaded` + `retry_after_seconds`; fair round-robin per principal. |
| Concurrency | 1 collection/tenant; 1 render; maintenance exclusive | — | Equivalent queued collections join the recorded job. |
| Submission response | ≤10 s to durable acceptance | — | Then the job continues detached. |
| Ordinary reads | one coherent SQLite read, 2 s | fixed | Timeout = error, never mixed snapshots. |
| Maintenance ops | 120 s | 1–3600 s | Compaction 600 s. |
| Range per query/report | ≤90 days | — | 30/90-day qualified at ≥1M scans (M6/M8). |
| Receipts batch | ≤200 records / 256 KiB | — | CLI wait ≤2 s total per command. |
| Revisions | interval 4 h, allowance 6 | interval ≥1 h; allowance ≤24 | §8. |
| Bundle | HTML ≤32 MiB, bundle ≤64 MiB | — | 50,000 summary cells, 10,000 detail rows. |
| Disk reserve | 2× DB bytes + 2×64 MiB + transfer reservations + 64 MiB | — | Publication pauses at reserve + 512 MiB; collection at reserve. |

### 6.1 Implementation status (M4: collection, credentials and schedules)

- **Collection executor** (`collection.rs`, `collection_start`):
  - Each job runs one archive collection through `airs_history_store::sync::run`, with
    the service's AIRS tenant, built from the `[airs]` secret files. A secret file is
    opened non-blocking (following a projected Secret's symlink), then required to be a
    regular file of at most 16 KiB, printable, validated by the CLI's tenant validator,
    and registered as a secret.
  - The run uses the configured tick ceilings (`tick_seconds`, `tick_sends` including
    OAuth, `tick_pages_per_source`), on a private current-thread runtime on the worker
    thread.
  - Polling makes zero Reports calls.
- **Selector.** Progress is the *latest* complete traversal (a supported-scope
  `traversed_as_of` segment) of each windowed source and application, taken at the
  traversal's target end (`until_ms`) capped by its observation time, so a historical
  or delayed traversal never advances progress past hours it did not read; today that is
  the `sessions` inventory listing, the only time-windowed traversal. An ordinary recent
  tick starts at the oldest of those, minus the 15-minute overlap: the narrowest
  selector, at least one hour and at most 24 hours, and never before the archive's
  bootstrap. Partial, observed, unavailable and falling-behind segments never advance
  progress, and a windowed source with no complete traversal means a 24-hour read.
  Progress keeps its scope: a `cloud` segment covers every application, while an
  `application` or `selection` segment covers only the applications it names. A
  tenant-wide tick needs a cloud traversal; an application tick needs progress for every
  requested application and starts at the oldest of them.
  Per-item and snapshot sources (details, results, inventory, profiles) are not
  windowed; their unfinished items are retained work.
- **Scope.** The resolved scope (concrete bounds, applications) and the credential
  revision the run is bound to are checkpointed with the run ID before any send,
  together with every archive run the logical job has made. The collector is always
  given those concrete bounds, never a null range, so every recovery and continuation
  collects exactly the checkpointed target.
- **Completeness.** A run succeeds only when its durable record is complete: state
  `complete`, no unfinished required source and no unconfirmed read. Collection runs use
  strict completeness, and anything else is `partial`. Results follow the frozen
  `result_collection`; its send counts are classified from the durable journal, with
  `oauth.token` rows counted as OAuth and every other row as management.
- **C4.** The run ID is checkpointed before any archive write or send. A resumed job
  looks its run up:
  - **no archive row:** the run is collected under that ID;
  - **`running`:** it is closed `interrupted`, and the job ends `partial` with the
    original bounds and applications (after the cooldown, denial and transient-token
    checks below);
  - **finished:** the job finishes from its durable record, `succeeded` only when that
    record is complete;
  - **ended in a credential denial:** the denial is recorded under the run's bound
    revision. If that is still the current revision, the job parks `needs_credentials`
    with no new attempt; only after a rotation does a new run start.
  - **ended in a transient token failure with no data:** the job parks `waiting_retry`,
    keeping its retry eligibility.
  - Recovery first restores any management 429 hold from the journal's latest
    durable refusal time (15 minutes from it; an existing longer hold is kept), then
    checks for a journaled denial, then for a transient token failure, and only then
    takes the `running` shortcut. A journaled denial in a `running` run is recorded and
    parks the job `needs_credentials`; a journaled transient token failure (OAuth 429,
    5xx or transport) with no API data parks it `waiting_retry` with its scope, its runs
    (their consumption) and its original deadline. Either way the run is closed
    `interrupted`. If recording the denial fails, the job still parks with no new
    attempt, and every other job is refused by the archive authority below.
  - Collection and enrichment share this reconciliation.
  - A job that waited (`waiting_retry` after a transient token failure, or
    `needs_credentials` before a rotation) is parked with a `continue` checkpoint
    holding its reason, scope and runs. When it is claimed again it makes a new run for
    the unfinished work. Enrichment follows the same rules, and keeps a checkpointed run
    ID that never reached the archive.
- **Cumulative allowance.** A logical job's traffic (API, OAuth, Results, Reports, every
  durable `reports` reservation and every unconfirmed read) is counted over all its runs; an unreadable run counts as the whole
  allowance. A continuation run gets only what remains of `tick_sends`. A spent
  allowance is `quota_exhausted` with no request. Enrichment's Reports transports stay
  within two per job across all its runs.
- **C2.** One control reservation per run. It is settled `sent` once the run is known
  to have reached the archive, and stays `uncertain` (consumed) whenever that cannot be
  established.
- **Credentials:**
  - A management API read refused with 401 gets exactly one explicit fresh-token
    replay at that read: SDK retries stay zero, and the SDK's lease invalidation means
    the replay carries a fresh token. A 403 is never replayed, and a refused token
    request is a credential outcome, not an API 401. A scanner Results or Reports 401
    (a static API key, which no renewal changes) is final: one transport, never
    replayed.
  - **Each logical read is one replay chain.** Every transport of one read (its first
    attempt, its single replay and the OAuth request either needs) carries the same
    `replay_chain` in its journal descriptor, with the read's full logical identity
    (application, session, scan and sub-request, interval, paging); distinct reads
    never share a chain. "A read refused twice" and "a 401 followed by a successful read
    of the same operation" are judged within one chain, so a distinct read neither
    answers nor refuses another's 401.
  - From the journal:
    - Any terminal API refusal (a 403, or a 401 whose single replay also failed, from
      the per-read journal), even when other reads returned data; or API reads all
      refused; or the token endpoint refusing the credentials with no API data: a
      recorded denial for (tenant, plane, revision). The job is `needs_credentials`, and
      that revision is never tried again. A first 401 recovered by its replay is not a
      denial.
    - A transient token failure (429, 5xx or transport) with no API data:
      `waiting_retry`, bounded by the job's deadline.
    - Any definitive token refusal (the token endpoint answering 401 or 403, with no
      token obtained after it in the run), whatever else the run read: a recorded
      denial. The collector never replays a token refusal.
    - An API 429: the plane is held for 15 minutes.
  - **The archive is the authority for the management hold.** At every run's
    admission (both executors) the hold is the longer of the control store's cooldown
    and the hold derived from the archive: the latest journaled API 429 plus 15
    minutes, or the archive's own management cooldown (a longer provider
    `Retry-After`, an OAuth 429), whichever ends later. A shorter `Retry-After` never
    shortens the service hold, and neither does a cancellation, an expiry or a failed
    control-store write. A held run waits (`waiting_retry`) with no reservation and no
    request; an unreadable archive or control store sends nothing. The control record
    is the persisted copy: written by the run that saw the 429 (a failed write is
    logged, `management_hold_deferred`), by recovery, by any admission that finds it
    missing and by the worker's reconciliation pass.
  - **The same authorities fence every transport inside a run** (both executors). The
    archive journal admits each management and OAuth transport in the transaction that
    records it `started`, and refuses it when:
    - the archive's management cooldown holds. A service run's API 429 commits that
      cooldown with the 429 itself, for at least the service's 15 minutes (a longer
      provider `Retry-After` is kept; a token-endpoint 429 keeps the provider's
      duration). A peer read already admitted finishes, but nothing is sent after it
      until the hold ends.
    - the run's journal already holds a definitive credential refusal: a token 401/403
      with no later token, an API 403, or an API read refused twice (its replay refused
      too). A single API 401 is not definitive, so its one accounted fresh token and
      replay are still admitted.

    Reports transports are not journaled. They keep their own gates: the daily
    allowance, two sends and ten IDs per job, and a stop at the first 429.
    Scanner Results and Reports transports are accounted per transport: one admitted
    batch of at most five IDs is one transport (`work_attempts.transport_id`), so the
    run's durable `results`/`reports` counts, the job result's `sends` and the job's
    cumulative consumption equal the transports sent, and every Reports transport keeps
    its own control reservation.
  - **The archive journal is the authority for a revision's denial.** Every run is
    bound to the credential revision it was admitted under, in the archive itself
    (`collection_runs` provenance `credential_revision`, written with the run before any
    send) and in its job's `collecting` checkpoint. A revision is denied when the
    control store records it, or when any run bound to it holds a definitive denial in
    the journal: a 403; a 401 not followed by a successful read of the same chain,
    once the run is finished, or at once when its single replay was refused too; or, in
    a finished run with no API data, any authentication refusal (OAuth included); or,
    whatever the run's state, a token refusal (OAuth 401/403) not followed by a token
    the run obtained later (a token started in the same millisecond does not count). A
    run still `running` with one unanswered API 401 is not a denial (its replay may
    still succeed). The check reads the revision's 16 most recent bound runs: admission
    refuses every new run of a revision once one of its runs shows a denial, so a
    denying run is always among them.
  - That authority is consulted at every admission point, whatever ended the job that
    made the run (a cancellation, a deadline expiry at claim or in the scheduler pass, a
    failed control-store write, a crash): before each run's first send (both
    executors, every fresh, continued, rotated or resumed run) and before a reload
    requeues `needs_credentials` jobs. An unreadable control store or archive sends and
    resumes nothing.
  - The control-store record (`credential_denials`) is the authority's persisted copy:
    written by the run that observed the denial (before its cancellation is honoured,
    so a canceled job stays `canceled` and the revision stays blocked), by recovery,
    by any admission that finds the journal's denial unrecorded, and by the worker
    tick's reconciliation pass for the configured revision. Each write is
    opportunistic; safety never depends on it. Canceled and expired jobs keep their
    `canceled` and `failed` states.
  - **Credential resumption.** A job parked `needs_credentials` records the revision
    it was denied under. At startup, at every worker pass and at a reload that changes
    the revision, every job parked under another revision (or under none recorded) is
    requeued, keeping its original deadline, its checkpointed scope and its
    consumption, unless the configured revision is itself denied (above). Each job
    resumes in one transaction, so a failed write leaves it parked for the next pass; a
    job parked under the configured revision stays parked (no churn).
- **Cancellation.** Every outbound admission (API reads, OAuth, replays, Reports) first
  checks the durable job state, so a cancellation committed between any two sends stops
  the next one. A watcher also interrupts in-flight work. The job ends `canceled`.
- **Schedules (M4.3):**
  - Each due UTC slot submits one service-actor job under a slot-derived key. Pending
    slots coalesce into one catch-up job; older slots are `skipped` and never fetched.
    Slots missed while nothing ran (downtime) are recorded as `skipped` history, at most
    672 per catch-up, only where the version in effect had the schedule enabled, each on
    that version's own interval grid (re-aligned at a new version's effective time; the
    versions are walked chronologically, so every intervening version is honoured). A
    quarantined store schedules nothing.
  - `schedules_set` and `schedules_disable` append effective-dated versions; nothing
    earlier is overwritten. A tick uses the version in effect at its slot, so a future
    change never suppresses the ticks before it. `schedules_list` shows the version in
    effect now.
  - Completion bookkeeping commits on its own.
  - The `daily_report` and `daily_enrichment` kinds act from M6. Until then
    `schedules_set` refuses them as `invalid_request`.
- **Enrichment (M4.4):**
  - The collector runs over a period with Reports enabled: deterministic selection,
    cached examples reused, serial sends, and a stop on 429. A spent admission day blocks
    the job by policy before any request.
  - Every Reports transport is charged at its own admission (a `ReadBudget` gate) to the
    America/Chicago admission day of that moment, through the durable, never-refunded
    allowance with its high-water guard. Each transport has its own durable `reports`
    reservation. Per job, the bound is two sends and ten IDs.
  - A resumed run succeeds only when it completed.
  - `enrichment_start` (report → period) waits for service reports (M6).
- **Receipts (M4.5):**
  - Validation of the whole batch comes before any effect. Attribution goes only
    through `[[receipt_associations]]`; a client application name is never stored.
  - The batch key is reserved, bound to its digest, before any archive write, so other
    records under that key conflict even if the first attempt failed.
  - The reservation also binds the batch's archive run, named by the key's complete
    scope (tenant, caller, recovery epoch, key), in the same write; every later attempt,
    including recovery after a crash, writes that run. A pending reservation from an
    earlier build without a binding is bound only from proven provenance (the
    scope-named run exists; nothing was written; or the epoch-less run in a store that
    never had a restore); otherwise it is refused `conflict` with no archive effect.
  - Counts come from the run's own first-acceptance record, written in the transaction
    that inserts each receipt, so a replay after a crash reports the same result; no
    timestamp decides ownership. A run written before that record existed and already
    holding receipts cannot have its counts proven: its batch is refused `conflict` with
    no archive effect and stays pending for explicit reconciliation.
  - A refusal-only batch never opens or creates the archive.
  - The service path writes under its own receipt budget (5 s, not the CLI's). A batch
    of up to 200 records is written in chunks whose normalized size stays within
    256 KiB, each in its own transaction of the same deterministic run; a replay of the
    batch is idempotent.
  - Fields are normalized against the service's complete secret registry (every
    configured credential file). When any of them cannot be loaded the batch is refused
    `credential_blocked` before any reservation or archive effect (fail closed).
  - Client-asserted receipts are kept as their own facts but never enter the
    authoritative projection: a client claim cannot change, or make conflicting, what
    the provider observed.

### 6.2 Implementation status (M6: historical daily reports and retrieval)

- **Evidence as of the cutoff.** A report version is reconstructed from exactly what was
  known at its evidence cutoff, in one archive read transaction:
  - summary, detail and coverage all use facts, promoted receipt facts and coverage
    segments first observed by the cutoff;
  - an event with any fact first observed later is re-projected, read-only, from its
    earlier facts;
  - receipt ownership is the one at the cutoff: a receipt promoted, demoted or
    reassigned later contributes to the event it belonged to then (summary and detail
    alike);
  - the 256 KiB per-event rebuild bound counts only those cutoff-qualified facts;
  - a later fact never changes an earlier version, and the +24 h final uses its own
    cutoff.
- **Job binding.** A version is published only by a report job of the tenant. Recovery
  (C6) reattaches a staged bundle only when its manifest's job is that tenant's report
  job for the same report and kind, and restores its agent-facing summary; any other
  bundle is quarantined.
- **Deleted content stays deleted.** A report with a `report` tombstone is never
  published into again. Both the executor (at its start) and the publication transaction
  refuse, so a revision queued before a deletion publishes nothing after it, whatever
  fence it acquired. A manual report for the same period is a new report and is allowed.
- **Missed days by calendar date.** A missed daily slot is `catch_up` when its America/
  Chicago date is before today's: a spring-forward day is 23 hours long, a fall-back day
  25. Revision opportunities keep their elapsed-hour arithmetic.
- **Verified reattachment.** Crash recovery (C6) reattaches a staged bundle only when its
  manifest names its report, version and job, lists every required file exactly once,
  and each listed file exists as a regular file with the declared size and SHA-256 (no
  other file present). Anything else is quarantined.
- **Completion events.** A version's `report_published` completion event commits in its
  publication transaction. Recovery repairs a missing event (idempotently), and a retry
  of the publishing job with identical evidence re-asserts it.
- **Agent summary.** The agent-facing summary carries the same full-cohort counts as the
  page, not the capped detail.

- **Report-artifact deletion** (`lifecycle_preview`/`lifecycle_execute` action
  `artifact_delete`, moved here from M5 by the §5.1 scope reconciliation):
  - The parameters are `artifact_version_ids` (1–100 report versions of the tenant). The
    preview names exactly what goes (versions, files, bytes, attributed assessments) and
    each affected report's latest-pointer move. It binds the **reports fence** (which
    advances with every deletion and prune), so a preview made before another deletion
    is stale and refused. An unknown or already deleted version is `not_found`.
  - Execution follows the lifecycle authority path (Administrator, direct fresh proof or
    a one-use approval, a durable keyed job). One control transaction:
    - advances the fence first (C7: a render in flight can no longer publish, and
      recovery never reattaches an unrecorded bundle);
    - removes each version's assessments, publication and version rows;
    - records a `report_version` tombstone and a `report` tombstone with the job;
    - moves each affected report's latest pointer to its newest remaining version, or
      clears it.
  - The bundle directories are then renamed out of the published tree (`.deleted-*`) and
    removed. Bundles are per version, so no other version's files are touched. A
    process that stops after the control commit leaves a deletion that recovery
    completes before any reattachment; a deleted version is never presented or
    retrievable (`reports_get`, `reports_list`, `artifacts_read` and
    `artifacts_download` do not find it). A keyed retry joins the same job; a repeated
    deletion of already tombstoned versions is a no-op success.
  - A report that lost a version is never revised automatically again, and the daily
    schedule never recreates its period (the report row remains). A manual report for
    the same period can still be created. Earlier downloads and external backups are not
    erased. Deletion makes no AIRS or Reports calls.

- **Report input** (`airs_history_store::report`): the archive's committed events
  observed in a half-open UTC period and first seen by the evidence cutoff, rebuilt from
  immutable facts. Profiles and applications are the latest observed at or before the
  period end. Coverage per category is conservative: `complete` only when every group is.
  - Events without an observed time are counted and stated as a gap, never placed in the
    period.
  - More than 10,000 events are truncated deterministically and stated, never
    extrapolated. This bounds the *detail* only.
  - A period must be a whole number of hours.
  - Profiles and applications are selected by observation link (`profile_runs`,
    `application_runs`), so a configuration observed again after a different one
    (A, B, A) is current again.
  - **Full-cohort summary**, in the same coherent read (budget 60 s): event counts by
    America/Chicago report date, application, verdict and action, and
    detector-positive event counts by detector (a detector observation with a
    malicious verdict or positive metadata). One summary cell is one table row; the
    budget is 50,000 cells. A plan that does not fit disables, in order, the detector
    filter, then the date filter, then the application filter. Verdict and action
    always fit. A disabled filter is stated as a gap and never computed from detail.
    The evidence digest binds the summary and the profile timeline.
  - **Profile timeline:** each profile's configuration current at the period start,
    then each observed difference (at most 1,000 spans). These are observation times,
    not effective change times.
- **Report page** (`report_html.rs`, service only; the CLI's Signal Brief renderer is
  unchanged):
  - Deterministic, self-contained HTML.
  - CSP `default-src 'none'`, with exactly one hashed stylesheet and one hashed
    script, Trusted Types required, and no style or event-handler attributes. Summary
    data is a non-executable JSON block with `<`, `>`, `&`, U+2028 and U+2029 escaped.
  - Filters (report date, application, detector, verdict, action) recompute cards,
    tables and per-date meters from the full-cohort summary. The detail table (at
    most 10,000 rows) is filtered as capped detail.
  - Profile matrix (profiles on rows, services on columns, explicit enablement only),
    profile timeline, coverage and gaps.
  - The no-script and print views show the base scope.
  - A page over 32 MiB is re-rendered without the detail table, stating why; one
    still over is `result_too_large`.
- **Bundles** (`reports.rs`): each version is rendered from the archive alone
  (`report.html` as above; `evidence.json` and `review.md` by the CLI's own
  serializer and review packet). It is staged privately (0700
  directories, 0600 files, fsynced), placed immutably under
  `reports/<report_id>/<version_id>/`, and recorded in one control transaction that moves
  the latest pointer.
  - The manifest has the frozen `manifest` shape. It binds the tenant job, period,
    version, timezone, analysis and redaction versions, completeness, gaps, predecessor,
    and each file's size and SHA-256.
  - Identical report-relevant evidence (a digest that excludes run identifiers and
    observation times) produces no new version.
  - Bundle bounds: HTML at most 32 MiB, bundle at most 64 MiB, at most 25 versions.
- **C5–C7:**
  - unpublished stages are discarded on recovery;
  - a placed bundle without its control row is reattached from its own manifest only
    when its report exists and the `reports` fence still matches, and is otherwise
    quarantined, never reattached;
  - the fence is rechecked before placement and inside the recording transaction, so a
    renderer that a prune or deletion overtook stops.
- **Daily reports** (`daily_store.rs`):
  - At 00:00 America/Chicago the `daily_report` schedule queues the previous local day:
    half-open, 23, 24 or 25 hours.
  - Missed days get one catch-up report each, serially, oldest first, at most 31 days
    back; catch-up reports have no revision window.
  - Revisions use opportunities at +4..+24 elapsed hours, coalesced to the latest due
    one, with that opportunity's cutoff (+24 stays +24 when late). The allowance is 6,
    and the final allowance is reserved for +24.
  - `daily_enrichment` is disabled until enabled. Once the day's report is queued and
    15 minutes have passed, it queues one enrichment job for the previous day.
  - Every submission is the service actor's, under a bounded slot-derived key.
- **Tools:**
  - `reports_create` (whole-hour range of at most 90 days): the report ID is derived
    from the caller and key, so a replay returns its job. Quotas are charged at admission
    and never refunded: `manual_reports_per_principal_hour` per rolling hour (with a retry
    hint) and `manual_reports_per_tenant_day` per Chicago day.
  - `reports_list` and `reports_get`: frozen manifests, a summary of at most 8 KiB, and a
    signed cursor bound to the filter and the version set.
  - `artifacts_read` (`review.md` or `summary.md`): pages of at most 6,144 characters,
    within the agent bound, under a cursor bound to caller, version, file and file hash.
  - `artifacts_download` (CLI bound): chunks of at most 64 KiB. Each call is
    reauthorized; the file is verified against its manifest hash, and the chunk and file
    SHA-256 are returned. A file that no longer matches is `storage_unavailable`.
  - `assessments_submit`: attributed to the caller's grant label, bound to the exact
    `review.md` SHA-256 (a stale hash is `conflict`), stored beside the version, never
    in it, and idempotent by key.
  - `enrichment_start`: resolves a report to its period for the M4 enrichment executor.
  - `history_query`: full-cohort counts with up to three groupings. `day` is the UTC
    date. Detector rows overlap and are not summed into the cohort. Display is capped at
    10,000 rows.
  - `schedules_set` now serves `daily_report` and `daily_enrichment`.
- **Completion events:** a `report_published` event is recorded with each publication.
- **Tenant isolation:** every report, version and artifact lookup is tenant-scoped. Reports
  are tenant-wide, readable by any Reader grant listing the tool; another tenant gets
  `not_found`.

## 7. Optional CLI (M0.1 freeze; implemented in M7)

### 7.1 Optionality

Without `--mcp`/`--record-history`, the CLI makes zero MCP calls, reads no MCP
state, opens no SQLite, starts no server and behaves exactly as 8.1. Explicit
selection never falls back to live collection. An unavailable endpoint, version
mismatch or authentication failure exits 1 with a clear message.

### 7.2 Grammar (frozen names)

```text
airs mcp login CONNECTION [--open] [--port N]
airs mcp logout CONNECTION
airs mcp status [CONNECTION] [--output pretty|json]
airs mcp connections list [--output pretty|table|json]
airs mcp connections set CONNECTION --url URL --client-id ID --service-tenant ID
        --expected-tsg TSG --approved-origin ORIGIN... [--role ordinary|admin] [--replace]
airs mcp connections remove CONNECTION --force
airs mcp confirm PREVIEW_ID --connection CONNECTION      # admin role; controlling terminal

airs runtime report --mcp CONNECTION [--since UTC --until UTC | --interval N --unit hours|days]
        [--app-id ID]... [--title TEXT] [--strict] [--output-dir DIR | --bundle DIR]
        [--wait | --no-wait] [--timeout 1..3600]
airs runtime sync --mcp CONNECTION [--since UTC --until UTC [--backfill]] [--app-id ID]...
        [--with-examples [--max-reports 1..10]] [--wait | --no-wait]
airs runtime history status --mcp CONNECTION
airs runtime history coverage --mcp CONNECTION [--since UTC --until UTC] [--app-id ID]...
airs runtime history query --mcp CONNECTION --since UTC --until UTC [--app-id ID]...
        [--group-by day|application|detector|verdict|action|profile]... [--max N]
airs runtime history reports --mcp CONNECTION [--since UTC --until UTC]
        [--kind daily|revision|manual|catch_up] [--max N]
airs runtime history download --mcp CONNECTION --version VERSION_ID
        [--file report.html|evidence.json|review.md]... --output-dir NEW_DIR
airs runtime history import --mcp CONNECTION --file PATH --expect-tsg TSG --expect-environment ENV_ID
airs runtime history export --mcp CONNECTION --file NEW_PATH
airs runtime history prune --mcp CONNECTION --before UTC [--app-id ID]... [--examples-only]
        [--delete-artifacts VERSION_ID...] [--execute PREVIEW_ID]
airs runtime history compact|backup|recover --mcp CONNECTION [--execute PREVIEW_ID]
airs runtime history restore --mcp CONNECTION --backup BACKUP_ID [--execute PREVIEW_ID]
airs runtime jobs list --mcp CONNECTION [--state STATE]... [--max N]
airs runtime jobs get|cancel --mcp CONNECTION JOB_ID
airs runtime schedules list --mcp CONNECTION
airs runtime schedules set --mcp CONNECTION --kind collection|daily_report|daily_enrichment
        --enabled true|false --effective-at UTC [--interval-minutes 5..60]
        [--revision-interval-hours 1..24] [--revision-allowance 1..24]
airs runtime schedules disable --mcp CONNECTION SCHEDULE_ID --effective-at UTC
airs runtime receipts upload --mcp CONNECTION --from-bulk-state PATH
airs runtime scan|bulk-scan|resume-poll <existing arguments> [--record-history CONNECTION]
```

Lifecycle commands without `--execute` only preview, printing the preview ID, digest
and expiry. `--execute` is valid only on an admin-role connection and is subject to
§3; on any other connection it is a usage error. Every scalar option rejects
repetition. `--wait` is the default for submissions; `--no-wait` prints the job ID
and exits 0 once admission is durable.

Validation order: clap usage → mutual exclusions → tenant read. `--from-evidence`
conflicts with `--mcp` (exit 2) before any tenant or network access. History, sync,
jobs and schedules commands without `--mcp` are usage errors (exit 2) that name the
optional service. `--history-dir` and `--no-history` are removed from direct
scan/bulk/resume and from service commands (D-row in M1). Ctrl-C/SIGTERM while
waiting exits 130/143, prints the known job ID, and does not cancel. If admission is
unresolved, the CLI prints the operation receipt instead.

### 7.3 Connection TOML (tenant file namespace `mcp`)

```toml
# Synthetic example.
[mcp]

[mcp.connections.goshawks]
url = "https://mcp.goshawks.io/ws-example/airs/mcp"   # gateway route, HTTPS only
clientId = "00000000-0000-4000-8000-000000000001"      # public client; no secret
serviceTenant = "airs-tsg-1234567890"                   # expected public service tenant
expectedTsg = "1234567890"
discoveryOrigins = ["https://mcp.goshawks.io", "https://idira.example"]
role = "ordinary"                                        # ordinary | admin
cache = "private"                                        # private | none (admin forces none)
revision = "00000000-0000-4000-8000-0000000000aa"       # generated per explicit set
```

The rules follow the credential contract §2/§4 pattern. Strict keys; names use
tenant-name syntax; at most 16 connections. `url` must be HTTPS with no
credentials/query/fragment (HTTP is allowed only on loopback, for tests). The first
namespace write takes a private no-clobber rollback snapshot
`.TENANT.pre-mcp.UUID.toml`. Older binaries (8.1, and 8.2 before M7) refuse the
`mcp` table: the M7 test runs a frozen pre-M7 binary against an isolated store.
Readers never write. An MCP-only tenant needs no AIRS secrets.

### 7.4 Private CLI state (generated, not configuration)

Root: `$XDG_STATE_HOME/airs-cli/mcp/` (absolute, else
`$HOME/.local/state/airs-cli/mcp/`), 0700, owner-checked, no symlinks. Separate
from the tenant store, bulk checkpoints and any SQLite. This is the documented
DESIGN carve-out: token refresh *writes* session state, but tenant/config readers
remain nonwriting.

- `sessions/<sha256(canonical resource ∥ issuer ∥ clientId ∥ connection revision ∥ subject)[..32]>.json`
  (0600, ≤64 KiB) holds
  `{"format": "airs-mcp-session/1", "resource", "issuer", "clientId", "connectionRevision", "credentials": {...}}`.
  Writes are temp + fsync + rename + directory fsync. Any binding change makes the
  session unusable and requires a new `login`. Refresh-token rotation is serialized by
  an exclusive `flock` on the sibling `.lock` (rmcp `acquire_refresh_guard`). A lost
  refresh response may require explicit `login`. `logout` removes only that session.
  Admin connections write no session file.
- `receipts/<connection>/<idempotency_key>.json`
  (0600, ≤4 KiB, ≤1,000 per connection) holds
  `{"format": "airs-mcp-op/2", "operation", "paramsDigest", "idempotencyKey", "epoch", "createdAt", "jobId"|null, "connectionRevision"}`.
  No token, no payload. A receipt is created before the first submission send and
  updated with the job ID. A job ID is stored only if it is well formed and carries no
  secret of the writing invocation. It is removed only after the job reaches a terminal
  state *and* the result was written to stdout. A receipt invalidated by a connection
  change stays listed as unresolved (`airs mcp status`) and is never silently replaced.
  A version-1 receipt predates the job-ID check: it stays listed (with its key and
  operation) but its job ID is never shown. `airs mcp status` fails, and removes
  nothing, when a receipt directory or receipt cannot be read.
- `previews/<connection>/<preview_id>.json` holds `{"format": "airs-mcp-preview/2", ...}`,
  written only after every field was checked for form and for the invocation's
  secrets. A version-1 preview record is never read; the preview is recreated.
- Every transferred file (downloads, report files and bundles, exports) is screened,
  chunk by chunk with overlap, for the invocation's secrets before any byte is written.
  A file that carries one is refused and nothing is published.
- One bounded authentication recovery may replay a submission with its original key.
  Retrieval and cancel retries never create jobs.

The loopback prototype (`airs-mcp-spike-cli`) proves the cache mechanics, PKCE, the
`state`/`iss` checks, refresh rotation, noninteractive denial and logout under
`airs-cli-spike/`.

### 7.5 Implementation status (M7)

**Implemented**, in the CLI and `crates/airs-mcp-client` (no SQLite, no server dependency;
`cargo tree -p airs-cli` gains the client and rmcp's client half only). DESIGN rows
D350–D361; loopback coverage in `tests/mcp_client.rs` (the real `airs` → a fixture gateway
and authorization server → the real `airs-mcp` server with its worker running).

- **Connections and sign-in** (§7.3, §7.4): `mcp connections set|list|remove`,
  `mcp login|logout|status`, the private session cache and operation receipts, as in the
  slice. Admin-role connections (`cache = "none"`) write no session: every command on one
  signs in afresh, held in memory for that command only.
- **Reads:** `runtime history status|coverage|query|reports`, `runtime jobs list|get|cancel`,
  `runtime schedules list`; complete results bounded at 256 KiB.
- **Submissions** (`runtime sync`, `runtime report --mcp`, `runtime schedules set|disable`,
  lifecycle execution): keys minted under the service's epoch; a receipt is durable before
  the first send and removed only after the terminal result was printed; **one bounded
  authentication recovery**: a token the gateway refuses at the epoch read or the send is
  refreshed once (never a sign-in) and the submission replayed with its original key.
- **`runtime report --mcp`:** `reports_create` (archive-only render), wait, then
  `--bundle-dir NEW_DIR` (three files) or `--output-file NEW_FILE` (HTML). `--from-evidence`,
  `--review-file`, `--max-events`, `--max-reports`, `--max-pages`, `--concurrency` and
  `--max-requests` conflict with `--mcp` (exit 2) before any tenant or network access;
  `--timeout` bounds the wait (default 600 s). `--output-file -` is refused with `--mcp`.
- **Retrieval** (§5.2, §5.5): `runtime history download` and the report bundle fetch
  64 KiB chunks, check each chunk's offset, length and SHA-256 and the file SHA-256 against
  the manifest, retry a chunk at most three times from its verified offset, and publish
  each file through a private `O_EXCL` temporary, hard link and directory `fsync` into a new
  0700 directory. "Resumable" means within one command: an interrupted command leaves no
  partial file and is rerun from the start.
- **Lifecycle** (§3): without `--execute`, a `lifecycle_preview` whose result (digest, proof
  nonce, expiry) is kept in a private record `previews/<connection>/<preview_id>.json`
  (0600, ≤4 KiB, ≤100 per connection). `--execute PREVIEW_ID` on an admin-role connection
  signs in with `max_age=0` and `airs_proof_nonce=<nonce>` added to the authorization
  request; the gateway is expected to sign the nonce (claim `admin.proof_nonce_claim`) and
  `auth_time` into the identity assertion. `airs mcp confirm` issues an approval the same
  way, and the previewing connection executes it with `--approval`. Export and import run
  the whole flow in one command (two and three sign-ins respectively).
- **Receipt retention:** `runtime receipts upload --from-bulk-state PATH` and
  `--record-history CONNECTION` on scan/bulk-scan/resume-poll (2 s budget, warning only,
  never an interactive sign-in), batched at 200 records/256 KiB under deterministic keys.

**Grammar revisions in M7** (to §7.2; each recorded in DESIGN §12):

- `history download --version-id VERSION_ID` replaces `--version`: the CLI prints its
  version for `--version` anywhere on the command line (TypeScript parity pre-scan).
- `mcp confirm` accepts `--digest SHA256 --proof-nonce HEX` when no local preview record
  exists (an agent's preview made outside this CLI), and `--open`/`--port`.
- Lifecycle commands accept `--approval APPROVAL_ID` with `--execute` on the connection that
  made the preview, and `--no-wait`; admin commands accept `--open`/`--port`.
- `runtime report --mcp` uses the existing `--bundle-dir` and `--output-file` destinations
  (not `--output-dir`/`--bundle`) and `--timeout` for the wait.
- `airs mcp confirm` and `--execute` do not require a controlling terminal: a pty is not a
  proof (§3 rule 3). The control is the gateway-signed, nonce-bound fresh proof.
- `--record-history` needs `--scanner NAME` on scan and bulk-scan (receipts are attributed
  only through the named scanner's association).

**Not delivered in this version:**

- The live gateway's signing of the fresh proof is unqualified (M0.6 P3: no `auth_time` or
  step-up). The loopback fixture models it; deployed services keep `[admin] mode =
  "disabled"` until M8 qualifies an option. Live gateway, Claude Code and packaged-install
  proofs are M8 (M8.3, M8.4).
- Download resumption across separate commands (see Retrieval).
- `--delete-artifacts` (artifact deletion) and allowance changes are refused online, as the
  service refuses them (§5.1).
- The frozen pre-M7 refusal test runs when `AIRS_PRE_M7_BIN` names a binary built by
  `scripts/build-pre-m7-cli.sh` (the 8.0.0 release commit); without it the test reports
  that it did not run.

## 8. Scheduling rules (frozen; executable in the core skeleton)

- Timezone `America/Chicago` from jiff's embedded tzdb. The tzdb version is recorded
  per report.
- The daily slot at local 00:00 reports the previous local day `[start, end)` in UTC.
  2026-03-08 is 23 h (06:00Z–05:00Z next day); 2026-11-01 is 25 h (05:00Z–06:00Z).
- Revision opportunities fall at integer multiples of `interval` strictly below
  +24 h, plus a final +24 h. Defaults give +4, +8, +12, +16, +20, +24; 7 h gives +7,
  +14, +21, +24; 1 h gives 24 slots.
- A changed-evidence revision at a non-final slot needs `used + 1 < allowance`; the
  final slot needs `used < allowance`. The last allowance is always reserved for the
  closing snapshot, including when the allowance is smaller than the slot count.
- Delayed opportunities coalesce to the latest one due. The final cutoff stays at +24
  elapsed hours: late processing filters evidence by commit time ≤ cutoff.
- The Reports admission day is the Chicago date of `max(now, stored high-water)`, so
  a clock rollback or a policy/timezone change cannot reopen an earlier day.
- Catch-up reports after downtime: one per missed slot, serially, from retained
  history, with no revision window and no old example fetches.

## 9. Storage, recovery and ownership (M0.4)

### 9.1 Two stores, no cross-database transaction

- **Control store** `control.sqlite3`. Tables: `epochs`, `jobs`,
  `job_params` (immutable canonical JSON + digest), `job_checkpoints`, `job_events`,
  `idempotency` (tenant, sub, client_id, operation, epoch, key → job ID, digest),
  `tombstones`, `send_reservations` (transport ID, job, source, credential revision,
  state `reserved|sent|uncertain|not_sent`), `allowance_days` (tenant, Chicago date,
  sends, IDs) plus `allowance_high_water`, `cooldowns`, `credential_denials`
  (plane, application, credential revision), `schedules` / `schedule_slots`,
  `previews` / `approvals`, `publications` (report version → manifest hash, deletion
  fence), `fences`, `completion_events`, `audit`. It holds no credentials.
- **Archive store**: the per-cloud `archive.sqlite3` (`airs-history-store`; schema
  version 2, version 1 upgraded as in §5.1). Writes are idempotent and identified by
  `operation_id`.
- **Control-store schema versions.** The control store is schema version 2 (M5:
  `upload_sessions.state` gains `removed`). Version 1 is the accepted-M4 schema, kept
  frozen (`control_v1.sql`). A version-1 store is migrated when it is opened or
  restored, only on a private copy and only when its structure is exactly version 1's:
  the one changed constraint is widened (every row already satisfies it) so the table
  definition is byte for byte version 2's, then the copy is checked in full (integrity,
  foreign keys, the version-2 fingerprint, tenant, epochs) and published, the original
  preserved under `preserved-<ms>-<uuid>/`. Jobs, checkpoints, epochs, idempotency,
  authority records, uploads and quotas are unchanged rows. Any other version-1
  structure, and any other version, is refused unchanged. `control-backup` /
  `control-restore` cover the control store only (a version-1 backup set restores
  through the same migration).
- Rules: never open a transaction in one store while holding a transaction in the
  other. Lock order is maintenance gate → control → archive. No network, rendering
  or user interaction occurs inside a transaction or the gate. DELETE journal, FULL
  synchronous, `secure_delete` ON, a 256 MiB allocator cap for the process, two-second
  coherent ordinary reads, ≤200 records/256 KiB write batches.

### 9.2 Crash-point inventory (M3/M4/M6 must test each)

| # | Window | Durable before | Recovery rule |
| --- | --- | --- | --- |
| C1 | Accept submission | idempotency + job + params + quota charge in one control txn | Lost response → same key returns the same job. |
| C2 | Before an AIRS send | `send_reservations` row `reserved` committed | On restart `reserved` → `uncertain` (consumed). Only proven no-send → `not_sent` releases it. |
| C3 | After a send, before the archive commit | reservation `sent`/`uncertain` | Requery from a supported head; archive dedupes by fact identity. |
| C4 | Archive batch committed, control checkpoint not | archive `operation_id` | On restart, look up `operation_id` in the archive and advance the checkpoint. Never re-send to rediscover. |
| C5 | Render staged, not published | staging manifest in a private staging dir | Verify the hash; publish only if the fence/generation is unchanged, else discard owned staging. |
| C6 | Bundle renamed, control `publications` not updated | immutable manifest on disk | Reconcile from the manifest if not tombstoned/fenced; else quarantine. Never reattach a fenced bundle. |
| C7 | Prune between fence advance and deletion | fence + tombstones first | Publication rechecks the fence immediately before activation; stale renderers stop. |
| C8 | Preview consumed / job created | single control txn (§3) | Retry resolves to the same job. |
| C9 | Backup set copying | coordinated snapshot under exclusive maintenance; lock released before copying off-volume | An incomplete set has no `complete` manifest and is never listed as a backup. |
| C10 | Restore staged | staged copy validated before replacement | The current control store keeps its reservations/audit. Restored jobs/schedules stay inactive. |
| C11 | Control store restored/lost | new epoch requires explicit Admin activation | Old keys → `control_restored`. Unknown cooldowns hold ≥24 h. Unknown attempts stay exhausted. Collection is quarantined until reconciliation. |
| C12 | Hot journal at startup | — | Guarded recovery on a verified copy under exclusive ownership. On failure: maintenance mode, data intact. Never delete or recreate a database. |

**Control store implementation status (M3).** `crates/airs-mcp/src/control.rs` implements
the schema of §9.1 (`control.sql`). Every state change is one control transaction
through the frozen §10 edge table (`jobs.rs`).

- **C1.** Submission writes the idempotency row, job, canonical parameters and digest,
  and the first event in one transaction. The same key and digest return the same job;
  a different digest is `idempotency_conflict`; another epoch's key is
  `control_restored`. An equivalent live collection joins the recorded job
  (`joined_existing`).
- **Queue bounds** (§6): live jobs per tenant and per principal. Beyond them a
  submission is `overloaded` with `retry_after_seconds`.
- **C2.** A send reservation is committed before any send. On restart `reserved` becomes
  `uncertain`, and both stay consumed. Only a proven no-send (`not_sent`) releases one,
  and a settled reservation never changes again. Tested with real process deaths
  mid-submission and after a reservation.
- **State machine.** Every write of a job's state goes through the central §10
  validator (`transition`); recovery and restore add no edge of their own. Tests check
  that every recorded event is a §10 edge.
- **Recovery** under exclusive ownership releases the claims of the previous owner's
  `running` jobs without changing their state. The next claim resumes such an orphaned
  job from its last checkpoint, which records the archive `operation_id` used by C4.
  Open reservations become `uncertain` (consumed).
- **Fair dispatch:** the principal whose last claim is oldest goes first, and orphaned
  running jobs resume before new work. The submitter's grant is rechecked at every claim
  under the configuration current at that claim; a revoked grant blocks the job by
  policy. A job past its deadline fails, recording a fixed `result_failure` and its gap
  in the same transaction.
- **Joining** (§6): an equivalent submission joins only the same submitter's job, and
  only while it is `queued`, `running` or `waiting_retry`. It never joins another
  principal's job, or one that is cancelling, blocked or awaiting credentials.
- **Allowances** (§6, §8): the Reports admission day is the Chicago date of
  `max(now, high-water)`. Charges are durable and never refunded, a clock rollback
  cannot reopen a day, and `quota_exhausted` names the wait to the next day.
- **C9.** `backup_into` writes a `VACUUM INTO` copy and publishes the manifest last. A
  set without a manifest is never restorable.
- **C10/C11 restore.** `restore` checks the manifest, size and SHA-256. It then
  validates the staged copy itself against the manifest and this service before
  changing or placing it: integrity, schema, tenant, required tables, referential
  integrity and the active epoch. It never overwrites an existing store. The restored
  store:
  - gets a new epoch pending explicit Administrator activation;
  - keeps job states, with claims released and open reservations `uncertain`;
  - enters recovery quarantine.
- **C11 loss.** The `control/` directory is marked `provisioned` once a store exists. A
  missing database in a provisioned directory is a loss, not a first start. The seed's
  epoch is never reused: a new store is built with a fresh epoch pending activation, in
  quarantine. Until activation every submission is `control_restored`, and keys from
  before the restore or loss stay `control_restored` afterwards.
- **Recovery quarantine** is persisted, and activation does not end it:
  - the worker claims nothing. A claim needs both an active epoch and no quarantine,
    whichever of activation and reconciliation comes first;
  - every non-terminal job's consumption is unknown, so its budget is spent: it is
    marked exhausted, can never reserve another send, and its next claim fails it with
    the recovery gap and `quota_exhausted`. Nothing removes the mark: not
    reconciliation, cooldown expiry or a clock change;
  - the current admission day's Reports allowance is exhausted, and the high-water mark
    moves to now;
  - every plane (management, scanner, reports, token) holds a cooldown of at least
    24 hours, including planes the backup recorded none for;
  - every schedule is disabled with an effective time, its rows and history kept.

  `control-reconcile` ends the quarantine. It is an explicit Administrator step, and in
  M4 it also requires the collection-journal reconciliation. The holds it placed stay
  until they expire or are changed explicitly, including the unknown-send charges.
- **Cooldowns at reservation:** `reserve_send` refuses a reservation on a plane that is
  cooling down (a 429 or a recovery hold) with `upstream_unavailable` and the wait,
  before any send.
- **Structure validation:** a restore copy, and an existing store's private copy, must
  match this version's schema exactly before anything is changed, placed or opened
  read-write. Every table, column, constraint and index is compared with a store freshly
  built from the schema; table names alone are not enough.
- **Policy-blocked jobs** are reconsidered every scheduler tick under the current
  configuration. A job within its deadline whose grant is restored is requeued through
  `blocked_policy → queued`, keeping its deadline and consumed sends. A job still
  blocked is left with no new event.
- **C12.** An existing store is first copied, with any `-journal`/`-wal`/`-shm`
  sidecars, into a private directory. Only that copy is opened read-write (hot-journal
  rollback and WAL replay happen there) and validated. A refused store's original bytes
  and sidecars are never touched; refusals are `maintenance_required`. When the
  original had sidecars, it is preserved under `preserved-<ms>-<uuid>/` and the
  verified, recovered copy is published in its place.
- **Job tools:**
  - `jobs_list`, `jobs_get` and `jobs_cancel` show only the caller's own jobs; another
    principal's job is `not_found`.
  - `jobs_list` pages bind a digest of the caller's whole filtered set, taken in the
    same read as the page, so a set that changes between pages is `conflict`. A page
    shrinks until its complete result fits the agent bound.
  - `collection_start` returns the durable job within the 10 s acceptance budget; its
    executor arrives in M4, and until then accepted collections stay queued.
  - Control-store refusals keep their `retry_after_seconds` through dispatch.
  - A job's recorded gaps appear in its view.
- **Request-side control access:**
  - The active epoch and cursor key are published once when storage opens. They change
    only through offline maintenance, under the exclusive owner lock.
  - Every request-side control transaction runs off the async runtime and counts lock
    queueing within its budget: 2 s for reads, 10 s for submission and cancellation.
    Past the budget it is refused `storage_unavailable` without running.
  - Request-side mutations (keyed submission, cancellation) are idempotent, so a retry
    observes any effect.
- **Worker** (`worker.rs`): one in-process worker per service, on its own OS thread,
  never on the async runtime that serves requests.
  - It starts only after every fallible startup step has succeeded. It is stopped and
    joined, including on an unwinding exit, before the data subtree's ownership lock is
    released, so no worker step outlives ownership.
  - Every 5 s it runs a scheduler pass:
    - due retries requeue;
    - waiting, blocked and credential-blocked jobs past their original deadline fail,
      with their result and gap recorded;
    - a cancel-requested job that no worker holds completes.
  - It then claims fairly, and only operations with a registered executor (others stay
    queued and are never blocked).
  - Executors reach AIRS only through reservations (C2) and persist progress only
    through checkpoints (C3/C4). Each checkpoint reports a requested cancellation, after
    which the executor makes no further send, and the run ends `canceled`.
  - Outcomes map to §10 edges. A retry records `retry_after_ms`, and no attempt is ever
    refunded.
  - C4: a resumed run reads its last checkpoint and archive `operation_id`, and must
    look the operation up in the archive rather than re-send. M4 must make the operation
    ID durable, or deterministically recoverable, before the archive commit. It must
    then test death between that commit and the checkpoint advance. The M4 collection
    executor implements the lookup. M3 registers no executor.
- **Authority transitions** (§3, `authority_store.rs`): each step of the §3 executable
  model is one control-store transaction, bound to the active epoch:
  - preview creation;
  - direct-proof execution;
  - approval issuance (no self-approval);
  - one-use, non-transferable approval execution;
  - `upload_admit` → owned upload session (begin, chunk, finalize, abort);
  - owner-only `import_finalize`.

  Consumption and job creation commit together or not at all. A retry with the same key
  returns the same job only for identical preview, digest, generation and approval
  bindings.
  - A preview stores its canonical action parameters, and the store computes the digest
    from them. The job they create carries them exactly (`action_params`), so an
    executor can act on them after a restart.
  - An `upload_admit` preview needs a complete, valid declaration (kind, length,
    SHA-256, expected TSG and environment, nothing else). Its upload must begin at the
    declared length and finalize with the declared SHA-256.
  - Authority executions pass the same transactional queue admission as ordinary
    submissions. An overload returns `overloaded` with its retry hint and rolls the whole
    transition back, consuming nothing.

  The MCP tools that call these transitions are M5.
- **Offline maintenance** (`airs-mcp control-backup | control-restore |
  control-activate-epoch | control-reconcile --config PATH ...`): each runs only while
  the service is stopped, under the data subtree's exclusive owner lock. Restore never
  overwrites a store; the operator moves a damaged one aside explicitly. These are not
  exposed over MCP (§5.1 scope reconciliation): the M5 lifecycle tools offer `compact`,
  `recover`, `prune`, `export` and `import_finalize` online, and refuse `backup`,
  `restore` and `epoch_activate` as offline maintenance.

### 9.3 Singleton ownership and filesystem shape

- One replica, `strategy: Recreate`, PVC `ReadWriteOncePod` on the Longhorn class
  (RWOP is already used by `incident-response` on `longhorn-replicated`). RWOP
  prevents a second pod from mounting the volume; RWO alone does not (same-node
  pods can share it). M0.6 demonstrated a second pod Pending, `Recreate` without
  overlap and a fresh owner ID per pod. Node-loss behavior without automated
  force-detach is demonstrated at M8.2 on a dedicated disposable node (plan revision
  7, amendment 3), never on a shared node. A negative result reopens the design.
- Inside the pod, an exclusive non-blocking `flock` on `DATA/owner.lock` plus a
  published `owner.json` (owner UUID, pod, node, PID, time) refuses a second process.
  **This is not distributed fencing**, and no part of the design claims it is.
- The admitted data subtree holds `owner.lock`/`owner.json`; `config/` (the private
  `active-config.toml` the service activates from); `history/` (the archive root of
  `airs-history-store`); and `control/`. In M2 `control/seed.json` (0600) holds the
  recovery epoch and the cursor key. It is created once with no-clobber publication,
  never regenerated over an existing file, and refused if malformed or loosely
  permissioned. The M3 control store adopts these values; it never re-mints them.
- The volume root is root-owned ext4 (0755, plus `lost+found`). There is **no
  `fsGroup`**: it would make admitted paths group-writable, and admission refuses
  that (proved by a loopback test with a setgid 2770 ancestor). A constrained init
  container (root, `CAP_CHOWN` only, `readOnlyRootFilesystem`) runs
  only `mkdir -p -m 0700 /data/airs && chown 65532:65532 /data/airs` if it is
  missing. It never recurses and never changes an existing subtree. The main
  container runs as UID/GID 65532, `runAsNonRoot`, read-only root filesystem, drops
  all capabilities, `seccompProfile: RuntimeDefault`, and
  `automountServiceAccountToken: false`. The admitted subtree has a root-owned
  non-writable parent chain; NFS/SMB is refused by `statfs` magic.

### 9.4 Backups (operator-owned policy; revised 2026-11-07)

- **Content:** a coordinated, SQLite-consistent set under exclusive maintenance:
  `VACUUM INTO` copies of control and archive(s), the artifact manifest list, and a
  `manifest.json`
  (`format: airs-mcp-backup/1`, `created_at`, `epoch`, `stores[{name, schema, sqlite_version, bytes, sha256, rows_digest}]`, `artifacts[{id, sha256, bytes}]`, `complete: true`).
  It is written to `DATA/backups/<id>/` with 0700/0600, no-clobber, and fsynced. The
  service keeps the newest three verified sets on its volume (`control-backup-set
  --keep 3`); nothing in the service deletes a set that a newer verified set has not
  superseded.
- **Where the data lives:** SQLite databases and report bundles on the service's own
  data volume. The service knows nothing about any backup target, mirror, NAS or object
  store. Carrying the volume (or the sets) off-volume, and the retention, deletion and
  access rules of those copies, are the **operator's** policy and the operator's
  environment, not part of this contract or of any phase gate.
- **Reference deployment (deploy/airs-mcp):** the data volume uses an encrypted
  StorageClass (dm-crypt; the passphrase delivered as a Secret the class references),
  so any volume-level backup the operator takes stays ciphertext. The manifests encode no
  backup target. The daily set job (runbook §9a) produces a verified set before the
  operator's volume backup, if any, runs: a **24 h RPO** when the operator backs the
  volume up daily. A **periodic isolated restore drill** (runbook §11) is recommended;
  the operator is accountable for the policy and the drill.
- **Key custody (when the encrypted class is used):** the passphrase lives in the
  operator's secret store (the reference deployment: Conjur
  `data/airs-mcp/backup/volume-passphrase`, delivered by ESO) with an offline escrow
  copy held by the operator. Losing both makes volume backups unrecoverable. This is
  stated plainly in the runbook.
- **History.** Until 2026-11-07 this section carried the reference environment's own
  backup chain (an NFS target mirrored to a second NAS and to an R2 bucket, with two
  "open checks": the second NAS's retention record and an inventory proving no
  agent-accessible credential could delete objects in the bucket). The owner withdrew
  them as acceptance gates: they described one environment, not the product. Operators
  with mirrored targets should still record the equivalent facts for their environment;
  `scripts/m8/check-r2-delete-rights.sh` remains available as an optional offline helper
  for Cloudflare R2 inventories.

### 9.5 Capacity model and initial PVC

B0 measured 756,043,776 bytes for 1M synthetic scans. At a planning size of 5M scans,
the archive is ≈3.6 GiB, with a 2× maintenance reserve of 7.2 GiB. Control is
<1 GiB. Artifacts: the worst case (7 × 64 MiB per day) is ≈160 GiB per year; a
realistic estimate of ~5 MiB × 7 per day is ≈12.5 GiB per year. The local backup set
is ≈1× the databases. **Initial data PVC: 100 GiB** (online expansion; publication
pauses at reserve + 512 MiB). Capacity alerts fire at 70%/85%. M8 re-derives the
size from measured growth.

### 9.7 M8 status note (source-side complete; user-run evidence outstanding)

- **Artifacts.** `deploy/airs-mcp/` holds the production GitOps artifacts:
  - a singleton `Recreate` Deployment, with no `fsGroup`, a CHOWN-only init container,
    and secrets projected read-only at 0400 and never in the environment;
  - a 100 Gi `ReadWriteOncePod` PVC on the encrypted class;
  - an `https`-only Service;
  - NetworkPolicies: default deny, 8443 from the gateway, 8081 from `monitoring`;
  - a cert-manager Certificate;
  - a configuration template, a kustomization with a placeholder image digest, and the
    image Dockerfile.

  `crates/airs-mcp/tests/deploy_manifests.rs` validates them statically against
  §9.3–§9.5 and §11, and through the strict configuration parser. Agents never applied,
  built or pushed them.
- **Metrics.** The private health listener also serves `/metrics`: fixed Prometheus
  series for readiness, maintenance, a bounded (2 s) control read, the ten job states,
  data-volume size and free bytes, and archive and control store bytes. It carries no
  tenant, principal, path or credential data. Capacity alerts at 70% and 85% (§9.5) use
  the volume series.
- **Backup-policy helpers (§9.4).** Optional, operator-run, offline.
  `scripts/m8/check-r2-delete-rights.sh` validates a sanitized credential inventory for a
  Cloudflare R2 bucket (strict schema in its header; exit 3 unknown, exit 4 violation).
  It is not an acceptance gate (owner decision, 2026-11-07).
- **M8.2 local qualification.** `qualify_restore_and_contention_at_scale` (opt-in) seeds
  the qualified dataset, runs report readers against a live writer, and restores and
  verifies a `VACUUM INTO` backup copy. It records peak RSS and I/O. Production storage,
  low-space recovery on the actual storage class, and the node-loss test on a dedicated
  disposable node are user-run.
- **M8.3 and M8.4.**
  - `docs/RUNTIME_MCP_DEPLOYMENT_RUNBOOK.md` and `scripts/m8/user-checklist.sh` cover
    the owner's proof: gateway sign-in, Claude Code, backup marker, node loss and restore
    drill.
  - `docs/RUNTIME_MCP_RELEASE.md` and `scripts/m8/build-hashes.sh` produce local,
    non-publishing builds and SHA-256 manifests. macOS signing and the container image
    are owner steps.

## 10. Jobs (frozen state machine)

States: `queued`, `running`, `waiting_retry`, `needs_credentials`, `blocked_policy`,
`cancel_requested`, `canceled`, `succeeded`, `partial`, `failed`.

| From | To | Trigger |
| --- | --- | --- |
| queued | running | worker claim, after recovery and ownership checks, with the grant rechecked |
| running | waiting_retry | token-endpoint 429/5xx/transport failure; respects cooldown and deadline |
| running | needs_credentials | second 401 after the single fresh-token replay, or a permanent 403/denial |
| running | blocked_policy | grant revoked, quota exhausted, quarantine, low space |
| running/queued/waiting_retry | cancel_requested | `jobs_cancel` |
| cancel_requested | canceled | next checkpoint, before new sends/publication/destructive steps |
| running | succeeded / partial / failed | terminal, with verified artifacts for succeeded/partial |
| needs_credentials | queued | validated same-scope credential revision; original deadlines/allowances still bind |
| waiting_retry | queued | retry time reached within caps |
| needs_credentials / blocked_policy | cancel_requested → canceled | `jobs_cancel`; no further sends occur in these states, so cancellation completes at the next scheduler pass |
| blocked_policy | queued | the blocking condition clears (grant restored, admission day rolled, quarantine reconciled, space freed), rechecked at claim time, within the job's original deadline |
| needs_credentials / blocked_policy / waiting_retry | failed | the job's original deadline passes (default 24 h for collections, 72 h for reports); committed evidence is kept and the result lists the gap |

Persist before returning a job ID: canonical immutable parameters and digest, scope,
policy/config/credential revisions, submitter or service actor, scheduled slot,
quota charges and reservations. Disconnect, HTTP cancellation and token expiry never
cancel a job. Uncertain sends stay consumed. There is no exactly-once claim for
external APIs. Management 401 gets exactly one accounted fresh-token replay at the
failed read; a 403 gets none. There are no whole-job retries and no stacked SDK
retries.

## 11. Server configuration (strict TOML)

The production `airs-mcp` file is the spike schema
(`prototypes/runtime-mcp/crates/airs-mcp-spike/src/config.rs`, enforced and tested
in M0) plus the AIRS and service sections below. Every table is `deny_unknown_fields`;
the file is ≤64 KiB; there are no environment-variable overrides.

```toml
revision = "prod-1"                       # public, non-secret; appears in audit/eligibility

[server]   # as the spike: listen, allowed_hosts, resource, authorization_servers,
           # scopes_supported, data_dir, [server.tls] cert_file/key_file, max_request_body_bytes

[upstream] # as the spike: issuer, audiences, tenant_id, jwks_url, algorithms (asymmetric),
           # accepted_permissions ⊆ {AirsHistory.Read, .Operate, .Admin},
           # allowed_client_ids, owner_object_ids (nbf/exp/iat required; delegated scp only)

[assertion]   # as the spike: header, issuer, audiences (backend binding), algorithms,
              # jwks_url XOR public_key_pem_file, max_lifetime_seconds (1..300),
              # workspace_ids, organization_ids, [assertion.claims] names

[admin]
mode = "disabled"                  # disabled | fresh_proof (§3)
# proof_nonce_claim = "..."; proof_max_age_seconds = 120   (required with fresh_proof)

[service]
tenant = "airs-tsg-1234567890"     # public service tenant ID (tool `tenant` field)
timezone = "America/Chicago"       # compiled default; other values are a contract revision

[airs]                              # one TSG at launch
tsg = "1234567890"                  # 1..32 digits
management_endpoint = "https://api.apps.paloaltonetworks.com/aisec"
dashboard_endpoint = "https://api.apps.paloaltonetworks.com/aisec"
token_endpoint = "https://auth.apps.paloaltonetworks.com/oauth2/access_token"
management_client_id_file = "/run/airs-mcp/secrets/mgmt-client-id"       # projected Secret files
management_client_secret_file = "/run/airs-mcp/secrets/mgmt-client-secret"
management_revision = "<uuid>"       # credential revision (§10); not derived from secret bytes

[[airs.scanners]]                    # credential contract §2 entry, with secrets by file reference
name = "monitor"
applicationId = "app-a"
endpoint = "https://service.api.aisecurity.paloaltonetworks.com"
numRetries = 0
revision = "<uuid>"
api_key_file = "/run/airs-mcp/secrets/scanner-monitor"   # exactly one of api_key_file/api_token_file

[[receipt_associations]]             # (caller sub, agent client_id, declared scanner) → binding
subject = "PLACEHOLDER_CALLER_SUB"
client_id = "PLACEHOLDER_AIRS_CLI_PUBLIC_CLIENT_ID"
declared_scanner = "monitor"
scanner = "monitor"

[limits]                             # optional overrides, each bounded by §6 ceilings
collection_interval_minutes = 15
tick_seconds = 120
tick_sends = 200
tick_pages_per_source = 40
reports_daily_sends = 2
reports_daily_ids = 10
manual_reports_per_principal_hour = 4
manual_reports_per_tenant_day = 24
queue_per_tenant = 64
queue_per_principal = 8
staging_quota_bytes = 1073741824

[[grants]]   # as the spike: subject, client_id, owner_object_id, permission, tools
```

Validation rules, beyond the spike's:

- Secret-file references must be regular files of at most 16 KiB. Their contents are
  validated with the CLI's scanner/management validators and registered in the
  per-job `SecretSet`. Their bytes are never logged or used as identity.
- Endpoints follow D48 admission: HTTPS, no credentials, query or fragment.
- `receipt_associations` names only configured scanners.
- Every `[limits]` value lies within §6's bounds.
- Every grant tool is in the catalogue, and its required tier is at most the grant's
  `permission` (M0.6: the spike accepted `operator_noop` in a Reader grant; calls were
  still refused, but `service_info` listed it). `service_info.tools` (the frozen
  schema field) is the callable set, tier ∩ grant tools, never the raw grant list.
- An Administrator grant lists only read-only
  tools and tools whose authority class is `direct_proof`,
  `direct_proof_or_approval` or `upload_session`; it never lists a mutating `grant`
  tool (§3.5).

Differences from the spike schema, decided in M2:

- **Removed:** the spike-only `server.max_echo_bytes`, `max_chunk_bytes`, `sessions`,
  `max_sessions` and `test_creation_delay_ms`, and `[route_binding]`. Resolutions 1 and
  2 are unavailable (§2.2); sessions would be a contract revision (§2.3).
- **Added:**
  - `server.health_listen`: the private, unauthenticated `/livez` and `/readyz`
    listener, with fixed bodies and no tenant data, never routed through the gateway;
  - `[service] maintenance`;
  - an optional public grant `label`, recorded in audit lines and defaulting to the tier
    name.
- Every grant subject, client ID, owner ID and label is diagnostic text: 1–128
  characters with no control character.
- Reloads (SIGHUP) cannot change the listener, health listener, `allowed_hosts`,
  `max_request_body_bytes`, TLS paths, data directory, service tenant, TSG or cloud
  endpoints. Those need a restart: the transport is built with them, and a reload can
  never move the archive identity under a running service.
- **Activation:** the source is read once as bounded bytes, and that exact candidate is
  fully validated (schema, restart-only settings on reload, verifier key sources).
  - Key sources are read only during that validation. The prepared configuration and
    verifier are what activation installs; nothing is reread afterwards.
  - The same bytes are staged as a private 0600 candidate beside the active copy, read
    back, compared, and renamed over the copy atomically.
  - Every fallible step that can refuse happens before the rename: opening the
    configuration directory, writing, syncing, reading back and comparing the staged
    candidate. A refusal leaves the previous private copy and the running configuration
    untouched, with no staged file left behind.
  - Once the rename has happened, the new copy is active. Activation then always installs
    the prepared configuration, so disk and memory agree. Nothing between publication and
    installation can fail or panic: diagnostics are emitted only after installation. A
    directory sync failing after the rename is logged (`config_publish_not_durable`),
    never reported as a refusal.
  - Logging and command output never panic or interrupt the service. A failed or closed
    stream drops the line, and a command keeps its exit status.
  - Startup selects storage from the same bytes it activates.
- `[limits]` overrides may lower defaults where §6 gives no higher ceiling (manual
  reports per principal ≤ 4, queues ≤ 64/8).
- The `[airs]` secret-file references are validated for form (absolute paths) in M2.
  Opening them, enforcing the 16 KiB bound and validating contents begin with
  collection in M4.

At startup the server reads the projected (symlinked) file, bounds and validates it,
publishes a private 0600 copy under the data subtree, and activates only from that
copy. Reloads are atomic after complete validation. Credential revisions are separate
from `revision`, so an unrelated reload cannot reopen rejected credentials. Errors
print fixed reasons, never parser text.

## 12. Prerequisites and decision rules before M1

| # | Item | Owner | Decision rule |
| --- | --- | --- | --- |
| P1 | Signed per-backend assertion binding (§2.2) | user/gateway | Resolution 1, 2 or 3 with a receipt; otherwise M1 is blocked. |
| P2 | Upstream `aud`/`scp`/`azp`/`oid` shapes and the resource/identifier-URI combination | user | Real sanitized shape receipt + denial of a mismatched case. |
| P3 | Admin authority option (§3) | user | A or B qualified; otherwise admin stays `disabled` (an acceptable outcome). |
| P4 | Grant tuples and allowed-tool lists | user | Written into the spike config; unknown agents denied. |
| P5 | CLI public client: loopback PKCE + refresh against the gateway's authorization server | user | `login`, `refresh`, `call`, an interrupted login and `logout` receipts. |
| P6 | Negotiated protocol and payload bounds through gateway and Claude Code | user | Receipts for ≤16 KiB complete pages and a ~175 KiB complete 64 KiB-chunk result, or a recorded smaller fallback chunk size. |
| P7 | RWOP + node-loss + fixed-UID subtree on Longhorn | user | Second pod Pending; ownership behavior recorded; failure reopens the design. |
| P8 | Encrypted backup target, key custody, RPO, retention, drill owner | user | Encrypted class + restore receipt + marker-absence check. |
| P9 | Cluster architecture | source: amd64 | The image targets linux/amd64 only until other platforms have evidence. |

### 12.1 M0.6 live outcome (2026-11-05; receipts `.local/dev/runtime-mcp/m0.6/receipts.md`)

| # | Outcome |
| --- | --- |
| P1 | Resolutions 1 and 2 are unavailable; route headers are not forwarded. Resolution 3 (upstream-token binding, §2.2) was **accepted by review 8 with conditions**, recorded as plan revision 7 amendment 1. Entra evidence for the conditions: the gateway client is confidential-only and is the only client consented to the API. The gateway API redacts OAuth settings, so client exclusivity rests on Entra plus the secret's provenance. Regression test: `org_wide_assertion_audience_is_bound_only_by_the_upstream_token`. |
| P2 | Pass. Live: real tokens satisfy every upstream check, and a deliberately mismatched upstream audience is `unauthenticated` (7.3b). The other negative upstream cases (issuer, tenant, `azp`, `scp`, `oid`, algorithm, lifetime) are loopback fixtures, not live. |
| P3 | Pass as **disabled**. The assertion has no `auth_time`/nonce; `AirsHistory.Admin` is not consented, so admin grants cap at Operator; an admin session from an agent shell cannot mutate. |
| P4 | Pass for denials: unknown caller and agent (simulated by grant changes, as disclosed), wrong workspace and over-tier tools are refused. Concurrency: with sessions, 15/15 requests from a second agent were refused (7.6). The stateless transport (§2.3, spike revision `m06-7`) then passed live through the gateway: reconnect (15/15 ok); concurrent with the admin CLI (Claude Code 15/15 ok; admin 258 ok and 1 client-side failure with no server denial; requests interleaved; 0 denials); no `Mcp-Session-Id` ever reached the backend; an ungranted tool was refused live (admin CLI `echo`, `forbidden`); and after a pod restart Claude Code recovered without reconnecting. The first call after the restart got a gateway 502 that never reached the new pod, cause unverified; calls 2 and 3 were ok. M1 consequence: clients and agents should treat a 502 from the gateway as retryable for reads, and mutations retry only through their idempotency keys (§3 rule 6). |
| P5 | Pass: login, refresh with rotation, call, interrupt, unapproved origin, logout. M7: a callback with a mismatched `state` must be ignored while the login keeps waiting, not abort it. |
| P6 | Pass. Protocol `2025-11-25`; ≤16 KiB pages; a 64 KiB chunk intact; 32 MiB in 512 calls (largest result 175,406 B). Claude Code spills results ≥ ~64 KiB to a file. |
| P7 | Pass for the second pod staying Pending, restarts not overlapping, a fresh owner ID and the UID 0700 subtree on ext4. **Node loss was not run**: the owner declined to take down a shared production node. Plan revision 7 amendment 3, accepted by review 9, moves it to M8.2, on a dedicated disposable node; P7 passes for the amended M0 scope, and node-loss fencing stays unpassed until M8.2. |
| P8 | Pass. Encryption: the volume is shown encrypted three ways, a wrong key is refused, and the correct key restores all 1,000 rows. The backup target, decompressed, holds no marker, no SQLite header and no ext4 names; written pages are at 8.00 bits/byte, and only the LUKS header is structured. Custody: Conjur, a 0600 local copy, and the owner's offline escrow. Encryption and restore: **pass**, live. Policy: **named and owner-accepted** (§9.4): target and the access role for each copy, a 24 h RPO, retention for each copy (nas2 14 daily plus 8 weekly; R2 plus 30 days), deletion rules, a monthly isolated restore drill, owner accountable. Two policy items stay **open, as M8 deployment checks** (plan revision 7 amendment 4, accepted by review 10), and are not claimed as passed: nas1's retention (unverified), and proof that no agent-accessible Cloudflare credential can delete R2 backup objects. |
| P9 | Pass: linux/amd64. |
