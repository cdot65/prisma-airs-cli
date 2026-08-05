---
sidebar_label: adapter
---

# redteam adapter

Manage **custom target adapters** — user-supplied scripts that run through a
network broker channel to reach targets no built-in connection type can. OAuth
credentials are shared with the other Red Team commands (`PANW_MGMT_*`).

### redteam adapter list

```text
airs redteam adapter list [--limit <n>] [--offset <n>] [--search <text>] [--output <format>]
```

List rows carry no script, description, or variables — use `get` for the full
record.

#### Examples

```bash
airs redteam adapter list
airs redteam adapter list --search keycloak --output json
```

### redteam adapter get

```text
airs redteam adapter get <uuid> [--output pretty|json|yaml]
```

#### Examples

```bash
airs redteam adapter get 3073d369-12e2-46c9-a45a-5697041fcbbf
airs redteam adapter get 3073d369-12e2-46c9-a45a-5697041fcbbf --output json
```

:::note Secrets are masked, not null

Secret variable values come back as the literal placeholder `'**********'`
with `is_redacted: true`. The CLI keys off the flag and renders `(redacted)` —
never treat the mask as the real value.

:::

### redteam adapter create

```text
airs redteam adapter create --name <name> --prompt <text> \
  (--script-file <path> | --script-b64 <b64>) \
  [--description <text>] [--channel <uuid>] [--variables <json>] [--draft]
```

| Flag | Required | Description |
|------|:--------:|-------------|
| `--name <name>` | Yes | Adapter name |
| `--prompt <text>` | Yes | Sample prompt used to exercise the adapter during validation (not stored) |
| `--script-file <path>` | One of | Script file — the CLI base64-encodes it for you |
| `--script-b64 <b64>` | One of | Script, already base64-encoded |
| `--channel <uuid>` | No | Network broker channel UUID — optional while DRAFT, required to activate |
| `--variables <json>` | No | JSON array of `{ "key", "value", "type": "VAR"\|"SECRET" }` |
| `--draft` | No | Save as DRAFT without running the validation script |

#### Examples

```bash
airs redteam adapter create --name my-adapter --script-file ./adapter.py \
  --channel 550e8400-... --prompt 'Hello' \
  --variables '[{"key":"endpoint","value":"http://agent.svc:8080","type":"VAR"},{"key":"api_key","value":"s3cret","type":"SECRET"}]'

# Draft first, activate later via update
airs redteam adapter create --name my-adapter --script-file ./adapter.py --prompt Hello --draft
```

### redteam adapter update

```text
airs redteam adapter update <uuid> --prompt <text> [overrides...] [--draft]
```

:::warning Upstream update is a full-replacement PUT

The API requires `name`, `script_b64`, and `prompt` on every update, and the
`variables` array defines the **complete** desired key set — an omitted key is
**deleted**. The CLI protects you: it reads the current record, merges your
overrides, and resends stored variables (secrets as `value: null`, which keeps
the stored value). Passing `--variables` replaces the whole set — include
every key you want to keep.

`--prompt` is required on every update because upstream never stores it.

:::

#### Examples

```bash
# Change only the description — stored variables (incl. secrets) are preserved
airs redteam adapter update 3073d369-... --description 'points at staging now' --prompt 'Hello'

# Swap the script and replace the variable set wholesale
airs redteam adapter update 3073d369-... --script-file ./adapter-v2.py --prompt 'Hello' \
  --variables '[{"key":"endpoint","value":"http://staging.svc:8080","type":"VAR"},{"key":"api_key","value":null,"type":"SECRET"}]'
```

### redteam adapter delete

```text
airs redteam adapter delete <uuid> [--force]
```

Alias: `rm`. Prompts for confirmation unless `--force`.

#### Examples

```bash
airs redteam adapter delete 3073d369-...
airs redteam adapter delete 3073d369-... --force
```

### redteam adapter validate

Run a script end-to-end through the broker channel using the sample prompt,
without saving anything.

```text
airs redteam adapter validate --channel <uuid> --prompt <text> \
  (--script-file <path> | --script-b64 <b64>) \
  [--variables <json>] [--adapter <uuid>]
```

- **The channel must be ONLINE** (network broker v1.4.0+). The CLI checks the
  channel status first and fails with a clear message instead of a confusing
  adapter error.
- **The full `variables` array is required by the run.** `--adapter <uuid>`
  resolves redacted/`null` values from the stored adapter *within what you
  send*; it does not supply the list. When you pass `--adapter` without
  `--variables`, the CLI fetches the adapter's variable set and sends it for
  you.
- The result is an **execution outcome** (`validated`, `stdout`, `stderr`,
  `traceback`), not an adapter record. On failure the CLI prints
  `stderr`/`traceback` — they are the useful part. Exit code is 1 when
  validation fails.

#### Examples

```bash
airs redteam adapter validate --script-file ./adapter.py \
  --channel 550e8400-... --prompt 'Hello' \
  --variables '[{"key":"endpoint","value":"http://agent.svc:8080","type":"VAR"}]'

# Re-validate an existing adapter's script, resolving its stored secrets
airs redteam adapter validate --script-file ./adapter.py \
  --channel 550e8400-... --prompt 'Hello' --adapter 660e8400-...
```
