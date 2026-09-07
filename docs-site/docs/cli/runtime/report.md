---
title: airs runtime report
---

# airs runtime report

Generate a read-only AI Runtime Security daily environment deliverable.

```bash
airs runtime report [--output html|markdown] [--output-file <new-path|->]
                    [--title <text>] [--max-pages <1-100>] [--strict]
```

| Option | Default | Purpose |
| --- | --- | --- |
| `--output` | `html` | Self-contained HTML or Markdown |
| `--output-file` | Timestamped, unique file in CWD | New destination; `-` streams to stdout |
| `--title` | `Daily environment report` | 1–240 characters |
| `--max-pages` | `40` | Page budget per source; 25 sessions/page, 100 application/configuration records/page |
| `--strict` | off | Exit 1 after generating the report if any source is incomplete |

Requires Management API OAuth credentials, not a Scanner key. Reads the existing config without
writing it. File output is mode `0600`, atomic and no-clobber. No automatic deletion of previous
reports. `--debug` and enabled SDK debug logging are refused for report privacy.

```bash
airs runtime report
airs runtime report --output markdown
airs runtime report --output-file ./runtime-daily.html --title "Operations review"
airs runtime report --output markdown --output-file - | less
```

See the [daily report guide](../../runtime/daily-report.md) for example HTML/Markdown deliverables,
live validation output, findings interpretation, evidence windows, and exit-code semantics.
