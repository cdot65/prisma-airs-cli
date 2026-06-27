---
title: Python SDK
---

# Python SDK

The Model Security Python SDK (`model-security-client`) provides programmatic access to scan local models and integrate with CI/CD pipelines.

## Install

Install the SDK with a single command. Authenticates via your management credentials and handles project/venv setup automatically:

```bash
# Install with all extras — auto-detects uv or falls back to python3 venv + pip
airs model-security install

# Install with only GCP extras into a custom directory
airs model-security install --extras gcp --dir my-scanner

# Preview what would run without executing
airs model-security install --dry-run
```

If `uv` is on PATH, the command runs `uv init` + `uv add`. Otherwise, it creates a `python3 -m venv` and uses `pip install`.

## PyPI Authentication

Get the raw authentication URL for Google Artifact Registry. For a fully automated setup, use `airs model-security install` instead:

```bash
airs model-security pypi-auth
```

## Common Workflows

### Customize Rule Enforcement

1. List rule instances in a group:
   ```bash
   airs model-security rule-instances list <groupUuid>
   ```

2. Change a rule from ALLOWING to BLOCKING:
   ```bash
   echo '{"state": "BLOCKING"}' > update.json
   airs model-security rule-instances update <groupUuid> <instanceUuid> --config update.json
   ```
