---
title: Security Groups
---

# Security Groups

### List groups

```bash
airs model-security groups list
```

```
  Prisma AIRS — Model Security
  ML model supply chain security

  Security Groups:

  bb1d038a-0506-4b07-8f16-a723b8c1a1c7
    Default GCS  ACTIVE  source: GCS
  020d546d-3920-4ef3-9183-00f37f33f566
    Default LOCAL  ACTIVE  source: LOCAL
  6a1e67e1-00cc-45dc-9395-3a9e3dbf50f9
    Default S3  ACTIVE  source: S3
  fd1a4209-32d0-4a1a-bd40-cde35104dc39
    Default AZURE  ACTIVE  source: AZURE
  4c22aef7-2ab7-40ba-b3f1-cd9e9aa1768e
    Default HUGGING_FACE  ACTIVE  source: HUGGING_FACE
```

### Filter groups by source type

```bash
airs model-security groups list --source-types LOCAL,S3
```

```
  Security Groups:

  020d546d-3920-4ef3-9183-00f37f33f566
    Default LOCAL  ACTIVE  source: LOCAL
  6a1e67e1-00cc-45dc-9395-3a9e3dbf50f9
    Default S3  ACTIVE  source: S3
```

### Get group details

```bash
airs model-security groups get bb1d038a-0506-4b07-8f16-a723b8c1a1c7
```

```
  Security Group Detail:

    UUID:        bb1d038a-0506-4b07-8f16-a723b8c1a1c7
    Name:        Default GCS
    Description: Auto-created default security group for GCS models
    Source Type: GCS
    State:       ACTIVE
    Created:     2025-10-30T14:14:17.321066Z
    Updated:     2026-02-19T16:03:26.660748Z
```

### Create a group

```json title="group-config.json"
{
  "name": "Custom S3 Group",
  "source_type": "S3",
  "description": "Custom security group for S3 model sources"
}
```

```bash
airs model-security groups create --config group-config.json
```

### Update and delete

```bash
airs model-security groups update <uuid> --name "Renamed Group"
airs model-security groups delete <uuid>
```
