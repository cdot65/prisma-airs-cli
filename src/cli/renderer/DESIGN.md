# CLI Output Design System

All terminal output flows through the primitives in `ui.ts`. Renderers compose
primitives; they do not call `chalk` directly. Command files call renderers (or
`ui.status()` for progress) — never `console.log` for user-facing text.

## Streams

| Stream | Carries |
|--------|---------|
| stdout | Data: pretty layouts, tables, and *only* the payload for `--output json\|yaml\|csv` |
| stderr | Everything that is not data: status/progress lines, warnings, errors, deprecation notices |

Rule: `airs <cmd> --output json | jq .` must always parse. Decorative output is
never emitted when a machine-readable format is selected.

## Semantic colors and glyphs

One meaning, one color, one glyph — everywhere:

| Semantic | Color | Glyph | Used for |
|----------|-------|-------|----------|
| success  | green  | `✓` | Completed action, passing check |
| error    | red    | `✗` | Failure, failing check |
| warn     | yellow | `⚠` | Degraded/retry/attention |
| info     | cyan   | `ℹ` | Neutral informational callout |
| skip     | yellow | `○` | Skipped / no-op item |
| flag     | yellow | `●` | Detection / policy hit |
| neutral  | dim    | `•` | Plain list item |

Headers are `bold` with **no color** — module identity comes from the title
text, not a per-module color. Labels/keys are `dim`; values are plain.

## Layout

- Baseline indent: two spaces for every content line.
- Header: blank line, `bold` title, optional `dim` subtitle, blank line.
- Section: `bold` label.
- Key/value block: `dim` key padded to the longest key, plain value.
- Table: the box-drawing style of `formatOutput` (`│` column separator,
  `─┼─` header rule) is canonical. No hand-rolled `padEnd` tables.
- Empty list: `dim` `No <resource> found`.

## Chalk & TTY

chalk v5 already disables color when stdout is not a TTY and honors
`NO_COLOR`/`FORCE_COLOR`. Do not add manual `isTTY` checks for coloring.
