# Model Security

Prisma AIRS CLI integrates with Palo Alto Prisma AIRS AI Model Security to manage ML model supply chain security. This enables scanning model artifacts for vulnerabilities, malicious code, and compliance issues before deployment.

## Overview

The `airs model-security` command group provides access to Model Security operations:

- **Groups** — manage security groups that define scanning policies per source type
- **Rules** — browse available security rules (read-only, managed by AIRS)
- **Rule Instances** — configure rule enforcement within groups (BLOCKING, ALLOWING, DISABLED)
- **Scans** — create, list, and inspect model security scans with evaluations, violations, and file results
- **Install** — one-command setup of the `model-security-client` Python package (auto-detects uv/pip, creates venv)
- **Labels** — organize scans with key-value labels
- **PyPI Auth** — get raw authentication URL for Google Artifact Registry

## Concepts

### Security Groups

A security group ties a **source type** (LOCAL, S3, GCS, AZURE, HUGGING_FACE) to a set of **rule instances**. Each group defines the security policy applied when scanning models from that source.

### Security Rules

Rules are the individual checks AIRS performs — unsafe deserialization detection, malicious code injection scanning, license compliance, etc. Rules are managed by AIRS and cannot be created or deleted via the API.

### Rule Instances

When a group is created, AIRS automatically provisions rule instances for all compatible rules. Each instance can be configured independently:

| State | Behavior |
|-------|----------|
| `BLOCKING` | Scan fails if rule triggers |
| `ALLOWING` | Rule evaluates but doesn't block |
| `DISABLED` | Rule is skipped entirely |

## Workflow

### 0. Install the Python SDK

```bash
# Auto-detects uv or pip, creates a project/venv, installs the package
airs model-security install

# Or install with only AWS extras
airs model-security install --extras aws
```

### 1. List available groups

```bash
airs model-security groups list
```

### 2. Browse security rules

```bash
airs model-security rules list
airs model-security rules get <uuid>
```

### 3. Configure rule enforcement

```bash
# View current rule instances in a group
airs model-security rule-instances list <groupUuid>

# Update a rule instance state
echo '{"state": "BLOCKING"}' > update.json
airs model-security rule-instances update <groupUuid> <instanceUuid> --config update.json
```

### 4. Create custom groups

```bash
echo '{"name": "Strict S3 Policy", "source_type": "S3"}' > group.json
airs model-security groups create --config group.json
```

### 5. Inspect scan results

```bash
# List scans
airs model-security scans list

# View evaluations for a scan
airs model-security scans evaluations <scanUuid>

# View violations
airs model-security scans violations <scanUuid>

# View scanned files
airs model-security scans files <scanUuid>
```

### 6. Organize with labels

```bash
airs model-security labels add <scanUuid> --labels '[{"key":"env","value":"prod"}]'
airs model-security labels keys
```

## CLI Reference

See [CLI Commands — model-security](../reference/cli-commands.md#model-security) for full option details.

See [Model Security Examples](../examples/model-security.md) for hands-on walkthroughs.
