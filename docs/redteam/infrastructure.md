---
title: EULA & Infrastructure
---

# EULA & Infrastructure

Manage the Red Team end-user license agreement, compute instances, devices, and container registry credentials.

## Prerequisites

- Prisma AIRS CLI installed and configured ([Installation](../getting-started/installation.md))
- AIRS management credentials set (`PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`, `PANW_MGMT_TSG_ID`)

---

## EULA

The Red Team EULA must be accepted before launching scans. Three subcommands manage the lifecycle.

### Check Acceptance Status

```bash
airs redteam eula status
```

Returns whether the EULA has been accepted, when, and by whom.

### View EULA Content

```bash
airs redteam eula content
```

Displays the full EULA text.

### Accept the EULA

```bash
airs redteam eula accept
```

Fetches the current EULA content and submits acceptance. This is a one-time operation per tenant.

---

## Instances

Red Team instances represent dedicated compute environments for running adversarial scans. Full CRUD is available via `airs redteam instances`.

### Create an Instance

```bash
airs redteam instances create \
  --tsg-id <tsgId> \
  --tenant-id <tenantId> \
  --app-id <appId> \
  --region <region>
```

| Flag | Required | Description |
|------|:--------:|-------------|
| `--tsg-id <id>` | Yes | Tenant Service Group ID |
| `--tenant-id <id>` | Yes | Tenant ID |
| `--app-id <id>` | Yes | Application ID |
| `--region <region>` | Yes | Deployment region |

### Get Instance Details

```bash
airs redteam instances get <tenantId>
```

Returns the instance configuration: TSG ID, tenant ID, app ID, and region.

### Update an Instance

```bash
airs redteam instances update <tenantId> \
  --tsg-id <tsgId> \
  --tenant-id <tenantId> \
  --app-id <appId> \
  --region <region>
```

### Delete an Instance

```bash
airs redteam instances delete <tenantId>
```

---

## Devices

Devices are attached to Red Team instances and represent the scanning infrastructure.

### Create Devices

```bash
airs redteam devices create <tenantId> --config devices.json
```

The config file contains the device specification as JSON.

### Update Devices

```bash
airs redteam devices update <tenantId> --config devices.json
```

Performs a PATCH update on the device configuration.

### Delete Devices

```bash
airs redteam devices delete <tenantId> --serial-numbers <serials>
```

| Flag | Required | Description |
|------|:--------:|-------------|
| `--serial-numbers <serials>` | Yes | Comma-separated serial numbers to delete |

---

## Registry Credentials

Fetch time-limited container registry credentials for pulling Red Team infrastructure images:

```bash
airs redteam registry-credentials
```

Returns a token and its expiry timestamp.
