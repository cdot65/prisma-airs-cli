---
title: Scans & Results
---

# Scans & Results

View and analyze model security scan results — evaluations, violations, and scanned files.

## List Scans

```bash
airs model-security scans list --limit 3
```

```
  Model Security Scans:

  7a7e1cdf-a6b1-4743-a5f2-a7bd96ec7bab
    BLOCKED  HUGGING_FACE  2026-03-03T22:32:12.344402Z
    https://huggingface.co/microsoft/DialoGPT-medium
    Rules: 10 passed  1 failed  / 11 total
  ee71b4da-64ce-4d6c-96fb-2bced1154a06
    ALLOWED  MODEL_SECURITY_SDK  2026-03-03T22:21:44.130386Z
    /Users/.../models/qwen3-0.6b-saffron-merged
    Rules: 6 passed  0 failed  / 6 total
```

## Filter Scans

```bash
# By evaluation outcome
airs model-security scans list --eval-outcome BLOCKED

# By source type
airs model-security scans list --source-type HUGGING_FACE
```

## Get Scan Details

```bash
airs model-security scans get <scanUuid>
```

## View Evaluations

See which rules passed or failed for a specific scan:

```bash
airs model-security scans evaluations <scanUuid>
```

## View Violations

```bash
airs model-security scans violations <scanUuid>
```

```
  Violations:

  0115bae0-bc07-468b-8ca6-a585d8c82f59
    Stored In Approved File Format  pytorch_model.bin
    Model file 'pytorch_model.bin' is stored in an unapproved format: pickle
    Threat: UNAPPROVED_FORMATS
```

## View Scanned Files

```bash
airs model-security scans files <scanUuid>
```

---

## Common Workflow: Investigate a Blocked Scan

1. Find blocked scans:
   ```bash
   airs model-security scans list --eval-outcome BLOCKED
   ```

2. View evaluations to find which rule failed:
   ```bash
   airs model-security scans evaluations <scanUuid>
   ```

3. View specific violations:
   ```bash
   airs model-security scans violations <scanUuid>
   ```

4. Check the scanned files:
   ```bash
   airs model-security scans files <scanUuid>
   ```

5. Look up remediation steps for the failed rule:
   ```bash
   airs model-security rules get <ruleUuid>
   ```
