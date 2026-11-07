---
title: mcp (optional service)
sidebar_position: 10
---

# `airs mcp` and the `--mcp` commands

Persistent runtime history, collection jobs and schedules belong to the optional
`airs-mcp` service, reached through your gateway. Nothing here is required: without
`--mcp CONNECTION` the CLI makes no MCP call, keeps no MCP state and opens no database.
Service commands without `--mcp` are usage errors (exit 2), and there is never a
fallback to live collection.

## Connections

Connections live in the selected tenant's file, in a strict `mcp` table. They hold no
secret: the CLI is a public OAuth client.

```bash
airs mcp connections set goshawks \
  --url https://mcp.example.com/ws-example/airs/mcp \
  --client-id 00000000-0000-4000-8000-000000000001 \
  --service-tenant airs-tsg-1234567890 --expected-tsg 1234567890 \
  --approved-origin https://mcp.example.com --approved-origin https://login.example.com
airs mcp connections list
airs mcp connections remove goshawks --force
```

- The URL must be HTTPS without credentials, query or fragment, and its origin must be an
  approved origin. Every sign-in endpoint the service advertises must be on an approved
  origin too.
- A tenant holds at most 16 connections. `--replace` changes an existing one and gives it a
  new revision, which signs out its session.
- The first `mcp` write saves a private rollback snapshot (`.TENANT.pre-mcp.UUID.toml`):
  older `airs` binaries refuse a tenant file with an `mcp` table.

## Sign-in

```bash
airs mcp login goshawks          # prints the sign-in URL; add --open to open a browser
airs mcp status [goshawks]       # connections, session presence, unresolved operations
airs mcp logout goshawks
```

- On an ordinary connection, `login` is the only interactive command. Every other command
  uses the saved session and fails with "authentication required" (exit 1) instead of
  prompting.
- An admin connection keeps no session. Each administrative command (`lifecycle
  --execute`, `history import|export`, `mcp confirm`) starts its own fresh sign-in and
  prints its URL, because the service requires a fresh, nonce-bound proof for every
  administrative mutation.
- Sessions are private files under `$XDG_STATE_HOME/airs-cli/mcp/` (else
  `~/.local/state/airs-cli/mcp/`): the directory is 0700 and the files 0600.
- A session is bound to the connection's URL, client and revision. Token values are never
  printed.

## Service commands

| Command | What it does |
|---|---|
| `runtime history status --mcp C` | archive presence, schema, last collection, gaps |
| `runtime history coverage --mcp C [--since UTC --until UTC] [--app-id ID]...` | per-source coverage (≤90 days; default the last 24 hours) |
| `runtime jobs list --mcp C [--state S]... [--max N]` | your jobs |
| `runtime jobs get\|cancel --mcp C JOB_ID` | one job; cancellation completes at the job's next checkpoint |
| `runtime sync --mcp C [--since UTC --until UTC --backfill] [--app-id ID]... [--no-wait]` | submit a collection; waits by default |
| `runtime schedules list --mcp C` | schedules, last completed slot, backlog |
| `runtime schedules set --mcp C --kind collection --enabled true --effective-at UTC [--interval-minutes N]` | enable or re-time the collection schedule |
| `runtime schedules disable --mcp C SCHEDULE_ID --effective-at UTC` | stop it from a time; history is kept |

- **Submissions.** `sync` and `schedules` mint an idempotency key under the service's
  recovery epoch. Before `sync` sends anything, a private operation receipt records the
  key, so a lost response is never resubmitted blindly.
- **Interruption.** Ctrl-C while waiting prints the job ID and leaves the job running.
- **Authentication recovery.** A token the gateway refuses during a submission is
  refreshed once and the submission replayed with its original key. Only `mcp login`
  and the administrative commands ever start a sign-in.

## Reports, downloads and retention

| Command | What it does |
|---|---|
| `runtime report --mcp C [--since UTC --until UTC \| --interval N --unit hours\|days] [--app-id ID]... [--title T] [--bundle-dir NEW_DIR \| --output-file NEW_FILE] [--strict] [--no-wait] [--timeout S]` | render on the service from its archive (zero AIRS calls); downloads are hash-verified and never overwrite |
| `runtime history reports --mcp C [--since --until] [--kind K] [--max N]` | report versions |
| `runtime history download --mcp C --version-id V [--file NAME]... --output-dir NEW_DIR` | a version's files into a new private directory |
| `runtime history query --mcp C --since UTC --until UTC [--group-by KEY]... [--max N]` | bounded aggregates, never payloads |
| `runtime receipts upload --mcp C --from-bulk-state PATH` | retain a bulk checkpoint's received results (no AIRS calls; retries are idempotent) |
| `runtime scan\|bulk-scan\|resume-poll … --record-history C` | also retain what the command received (2 s budget; a failure only warns) |

`--record-history` and `receipts upload` need a named scanner (`--scanner NAME`); the
service attributes receipts only through its configured association for that scanner.

## Administrative commands

Administrative actions run on an **admin-role** connection (`--role admin`). It keeps no
session: every command signs in afresh, and every mutation's sign-in carries the
preview's nonce and asks for a fresh authentication, which the gateway signs into the
identity assertion. The service accepts nothing else as proof.

| Command | What it does |
|---|---|
| `runtime history prune --mcp C --before UTC --examples-only [--app-id ID]...` | preview an examples-only prune (nothing changes) |
| `runtime history compact\|backup\|recover --mcp C` / `restore --mcp C --backup ID` | preview the action |
| `… --execute PREVIEW_ID` (admin connection) | execute that preview |
| `airs mcp confirm PREVIEW_ID --connection ADMIN` | approve a preview another actor made |
| `… --execute PREVIEW_ID --approval APPROVAL_ID` (the previewing connection) | execute an approved preview |
| `runtime history export --mcp ADMIN --file NEW_PATH` | the whole archive, verified, to a new private file |
| `runtime history import --mcp ADMIN --file PATH --expect-tsg TSG --expect-environment ENV` | a staged, validated import |

The deployed gateway does not yet sign this proof, so a service runs with administrative
mutation disabled until the deployment qualifies it (M8).
