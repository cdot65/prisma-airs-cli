---
title: Official Gateway API alignment
---

# Official Gateway API alignment

CLI 7.2 adds explicit organisation guardrails using SDK 0.34.0. It follows the
[official Prisma AIRS Gateway specification](https://github.com/PaloAltoNetworks/openapi/tree/2931d49bc38e30793d5923d22125f5f953d141d0),
while preserving working SCM deployment adapters. This is selective alignment,
not a claim that all 187 documented operations are implemented or live-verified.

## Choose the intended scope

`aigateway guardrails` manages workspace policies. `aigateway admin-guardrails`
manages organisation policies through the admin endpoint. Both use management
OAuth from the selected CLI tenant; neither uses Harness company SSO or a
workspace inference key. A Harness environment does not select the CLI tenant.

```bash
airs-cli tenant list
airs-cli aigateway admin-guardrails list --page-size 100 --current-page 0 --output json
airs-cli aigateway admin-guardrails get GUARDRAIL_UUID --output json
```

Pages start at zero, with a page size from 1 to 1000. `--workspace UUID` is an
optional filter, and can be denied by deployment policy. Organisation records
may have no workspace. Do not switch scopes after a 403 to work around a denial.

## Mutations and MCP mappings

Admin list has read-only live acceptance. Admin mutations and nonempty records
are specification-tested; deployment permissions and behavior still require
validation for the intended tenant. Inspect help and the target before a change.

```bash
airs-cli aigateway admin-guardrails create --help
airs-cli aigateway admin-guardrails update GUARDRAIL_UUID --name "Reviewed policy"
airs-cli aigateway admin-guardrails mcp-servers list GUARDRAIL_UUID --output json
```

Create accepts `--name`, `--target llm|mcp_tools`, optional `--workspace` and
`--organisation`, `--checks` and `--actions`. Organisation means a UUID, not the
numeric TSG. Structured `--file`, `--set` and `--set-string` input are also
available. Update cannot change scope. Delete requires confirmation or `--force`.

`mcp-servers sync` replaces all mappings and requires confirmation or `--force`.
The request is an object such as `{"mcp_servers":{}}`; that example removes every
mapping. `mcp-servers upsert GUARDRAIL_UUID SERVER_UUID` changes one mapping,
with `run_on` containing `input` and/or `output`, and optional capability UUIDs.
These commands configure policies. They do not authenticate an MCP session.
In Harness, use `/mcp` for gateway-facing OAuth/CAS sign-in.

## Preserved deployment behavior

The September 27 read checks returned 200 for workspace guardrails, service/user
key lists and the organisation guardrail list. The combined API-key collection
returned 403, as did the tested admin workspace filter. Existing
`aigateway api-keys service` and `user` commands remain the supported adapters.
Secret output remains redacted unless explicitly requested by the existing
key workflow. No keys were created or rotated during acceptance.

The new spec omits some working workspace/prompt/telemetry extensions; omission
is not deprecation. It also assigns some model, feedback and pricing paths to a
different API plane. This release does not reroute credentials or resurrect
legacy Assistants/Threads operations based on those declarations.

Inside Harness, use `airs cli ...` in the terminal or the absolute managed CLI
path provided to agent tools. `/config` and `/model` select conversation inference
routing; they do not change product tenants or admin policy scope. Native SSO,
MCP authorization, and the approved Jev credential handoff are preserved.
