# runtime deployment-profiles

## runtime deployment-profiles list

List deployment profiles

```text
airs runtime deployment-profiles list [options]
```

### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--unactivated` | No | — | Include unactivated profiles |
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

### Examples

*Pretty output (default)*

```bash
airs runtime deployment-profiles list
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management


Deployment Profiles:

  example-deployment-profile-1  init  D0000001
  example-deployment-profile-2  activated  D0000002
  example-deployment-profile-3  activated  D0000003
```

*JSON output*

```bash
airs runtime deployment-profiles list --output json
```

```text
[
  {
    "name": "example-deployment-profile-1",
    "status": "init",
    "authCode": "D0000001"
  },
  {
    "name": "example-deployment-profile-2",
    "status": "activated",
    "authCode": "D0000002"
  },
  {
    "name": "example-deployment-profile-3",
    "status": "activated",
    "authCode": "D0000003"
  }
]
```

*YAML output*

```bash
airs runtime deployment-profiles list --output yaml
```

```text
name: example-deployment-profile-1
status: init
authCode: D0000001
---
name: example-deployment-profile-2
status: activated
authCode: D0000002
---
name: example-deployment-profile-3
status: activated
authCode: D0000003
```
