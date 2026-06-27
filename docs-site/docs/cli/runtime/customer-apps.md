---
sidebar_label: customer-apps
---

# runtime customer-apps

### runtime customer-apps list

List customer apps

```text
airs runtime customer-apps list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--limit <n>` | No | `100` | Max results |
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

#### Examples

*Pretty output (default) — only `name` is rendered; richer detail requires `customer-apps get` (currently blocked, see cdot65/prisma-airs-cli#115)*

```bash
airs runtime customer-apps list --limit 2
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management


Customer Apps:

  example-app
  example-other-app
```

*JSON output — `id` and `description` are empty in the list response (the list endpoint only returns names)*

```bash
airs runtime customer-apps list --limit 2 --output json
```

```text
[
  {
    "id": "",
    "name": "example-app",
    "description": ""
  },
  {
    "id": "",
    "name": "example-other-app",
    "description": ""
  }
]
```

*YAML output (multi-doc stream — one document per app)*

```bash
airs runtime customer-apps list --limit 2 --output yaml
```

```text
id:
name: example-app
description:
---
id:
name: example-other-app
description:
```

---

### runtime customer-apps get

Get customer app details

```text
airs runtime customer-apps get [options] <appName>
```

#### Arguments

- `appName` (required) —

#### Examples

:::warning[Example needed]
No curated input/output example for this command yet.
:::

---

### runtime customer-apps update

Update a customer app

```text
airs runtime customer-apps update [options] <appId>
```

#### Arguments

- `appId` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--config <path>` | Yes | — | JSON file with app updates |

#### Examples

:::warning[Example needed]
No curated input/output example for this command yet.
:::

---

### runtime customer-apps delete

Delete a customer app

```text
airs runtime customer-apps delete [options] <appName>
```

#### Arguments

- `appName` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--updated-by <email>` | Yes | — | Email of user performing deletion |

#### Examples

:::warning[Example needed]
No curated input/output example for this command yet.
:::

---

### runtime customer-apps consumption

Show per-app token consumption + violation breakdown (SCM dashboard). Omit appName to scan all apps.

```text
airs runtime customer-apps consumption [options] [appName]
```

#### Arguments

- `appName` (optional) — Dashboard application name — the literal scan-payload metadata.app_name, as shown in the SCM AI Applications view (may differ from the SCM-registered customer-app name). Omit to report every dashboard bucket.

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--time-interval <n>` | No | `30` | Window in days: 7, 30, or 60 |
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

#### Examples

*Pretty output (default) — single app, default 30-day window. Only firing detectors are shown.*

```bash
airs runtime customer-apps consumption example-app
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management


example-app  (00000000-0000-0000-0000-000000000001)
  Monitoring since: 2026-05-25T16:42:52Z
  Source:           api
  Cloud:            other
  Profiles:         AI Gateway - Strict

  Token consumption:
    Daily avg:     37
    Monthly total: 37

  Sessions:
    Total:     2
    Violating: 1

  Detectors (1 violating, 1/8 firing):
    tc                       1  c=0 h=0 m=1 l=0
```

*Table output — one row per detector, app-level context repeated on every row.*

```bash
airs runtime customer-apps consumption example-app --output table
```

```text
App         │ AppId                                │ MonitoringSince      │ DailyAvg │ MonthlyTotal │ Sessions │ Violating │ Detector       │ C │ H │ M │ L │ Total
────────────┼──────────────────────────────────────┼──────────────────────┼──────────┼──────────────┼──────────┼───────────┼────────────────┼───┼───┼───┼───┼───────
example-app │ 00000000-0000-0000-0000-000000000001 │ 2026-05-25T16:42:52Z │ 37       │ 37           │ 2        │ 1         │ agent_security │ 0 │ 0 │ 0 │ 0 │ 0
example-app │ 00000000-0000-0000-0000-000000000001 │ 2026-05-25T16:42:52Z │ 37       │ 37           │ 2        │ 1         │ dbs            │ 0 │ 0 │ 0 │ 0 │ 0
example-app │ 00000000-0000-0000-0000-000000000001 │ 2026-05-25T16:42:52Z │ 37       │ 37           │ 2        │ 1         │ dlp            │ 0 │ 0 │ 0 │ 0 │ 0
example-app │ 00000000-0000-0000-0000-000000000001 │ 2026-05-25T16:42:52Z │ 37       │ 37           │ 2        │ 1         │ malicious_code │ 0 │ 0 │ 0 │ 0 │ 0
example-app │ 00000000-0000-0000-0000-000000000001 │ 2026-05-25T16:42:52Z │ 37       │ 37           │ 2        │ 1         │ pi             │ 0 │ 0 │ 0 │ 0 │ 0
example-app │ 00000000-0000-0000-0000-000000000001 │ 2026-05-25T16:42:52Z │ 37       │ 37           │ 2        │ 1         │ source_code    │ 0 │ 0 │ 0 │ 0 │ 0
example-app │ 00000000-0000-0000-0000-000000000001 │ 2026-05-25T16:42:52Z │ 37       │ 37           │ 2        │ 1         │ tc             │ 0 │ 0 │ 1 │ 0 │ 1
example-app │ 00000000-0000-0000-0000-000000000001 │ 2026-05-25T16:42:52Z │ 37       │ 37           │ 2        │ 1         │ uf             │ 0 │ 0 │ 0 │ 0 │ 0
```

*CSV output — pipe straight into spreadsheets or BigQuery. Self-contained rows; no join needed.*

```bash
airs runtime customer-apps consumption example-app --output csv
```

```text
App,AppId,MonitoringSince,DailyAvg,MonthlyTotal,Sessions,Violating,Detector,C,H,M,L,Total
example-app,00000000-0000-0000-0000-000000000001,2026-05-25T16:42:52Z,37,37,2,1,agent_security,0,0,0,0,0
example-app,00000000-0000-0000-0000-000000000001,2026-05-25T16:42:52Z,37,37,2,1,dbs,0,0,0,0,0
example-app,00000000-0000-0000-0000-000000000001,2026-05-25T16:42:52Z,37,37,2,1,dlp,0,0,0,0,0
example-app,00000000-0000-0000-0000-000000000001,2026-05-25T16:42:52Z,37,37,2,1,malicious_code,0,0,0,0,0
example-app,00000000-0000-0000-0000-000000000001,2026-05-25T16:42:52Z,37,37,2,1,pi,0,0,0,0,0
example-app,00000000-0000-0000-0000-000000000001,2026-05-25T16:42:52Z,37,37,2,1,source_code,0,0,0,0,0
example-app,00000000-0000-0000-0000-000000000001,2026-05-25T16:42:52Z,37,37,2,1,tc,0,0,1,0,1
example-app,00000000-0000-0000-0000-000000000001,2026-05-25T16:42:52Z,37,37,2,1,uf,0,0,0,0,0
```

*JSON output — one object per detector per app; full app context repeated for self-contained records.*

```bash
airs runtime customer-apps consumption example-app --output json
```

```text
[
  {
    "app_name": "example-app",
    "app_id": "00000000-0000-0000-0000-000000000001",
    "monitoring_since": "2026-05-25T16:42:52Z",
    "daily_avg": "37",
    "monthly_total": "37",
    "sessions_total": 2,
    "sessions_violating": 1,
    "detector": "tc",
    "critical": 0,
    "high": 0,
    "medium": 1,
    "low": 0,
    "total": 1
  }
]
```

*YAML output — multi-doc stream, one document per detector per app.*

```bash
airs runtime customer-apps consumption example-app --output yaml
```

```text
app_name: example-app
app_id: 00000000-0000-0000-0000-000000000001
monitoring_since: 2026-05-25T16:42:52Z
daily_avg: '37'
monthly_total: '37'
sessions_total: 2
sessions_violating: 1
detector: tc
critical: 0
high: 0
medium: 1
low: 0
total: 1
```

*Alternate time window — `--time-interval` accepts 7, 30, or 60 days (server-enforced enum).*

```bash
airs runtime customer-apps consumption example-app --time-interval 60
```

*All-apps loop — omit `appName` to enumerate every dashboard application bucket (one per distinct scan-payload `app_name`, so a single registered customer-app can appear as several buckets). Errors on individual apps are reported per-app; the loop continues past failures. Zero-traffic apps render `no detector violations in window`.*

```bash
airs runtime customer-apps consumption
```

*Invalid time-interval rejected client-side before incurring an API call.*

```bash
airs runtime customer-apps consumption example-app --time-interval 14
```

```text
Error: --time-interval must be 7, 30, or 60 (the API rejects other values)
```

*Unknown name lists the available dashboard application names and explains that the name to use is the literal scan-payload `metadata.app_name` (which may differ from the SCM-registered customer-app name).*

```bash
airs runtime customer-apps consumption no-such-app
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management


Error: Dashboard application not found: "no-such-app". Available (as shown in SCM AI Applications view): example-app, Claude Code, LiteLLM, Portkey, chatbot. Note: the name to use is the literal value your integration sends in scan metadata.app_name (which may differ from the SCM application name).
```
