# Redaction Rules

Apply these rules to every captured input and output before committing it to
the docs or to a fixture under `docs/cli/examples/`. Fixtures use the same
redactions.

## Required substitutions

- **Tenant service group ID** → `<tenant-id>`
- **UUIDs** → `00000000-0000-0000-0000-000000000001` (increment the last
  hex digit per distinct entity so relationships in the example stay
  legible: `...0001`, `...0002`, `...0003`, …)
- **Real API keys / client secrets** → MUST be absent. They are never in
  `airs` CLI output, but spot-check captures and fixtures anyway.
- **Real dictionary keyword content** — if sensitive (PII regexes,
  customer-specific terms, internal product names) swap for an innocuous
  fixture value.
- **User emails** → `user@example.com`
- **Real customer app names** → `example-app`

## Spot-check before commit

1. Search the new content for the real tenant ID (it should not appear).
2. Search for any real customer / project / product names that leaked in
   through `name`, `description`, or `tags` fields.
3. Confirm UUIDs match the `00000000-0000-0000-0000-00000000000N` pattern.
4. Confirm emails use the `@example.com` domain.

## Where redacted data lives

- **Inline in doc pages**: under `#### Examples` blocks in
  `docs/cli/runtime/**`, `docs/cli/redteam/**`,
  `docs/cli/model-security/**`.
- **Fixture files**: `docs/cli/examples/<area>/<command>.json` (referenced
  from doc examples via `--body-file`).

Same rules apply to both.
