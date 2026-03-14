# Model Security Operations

This guide walks through managing AI Model Security using the `airs model-security` command group — security groups, rules, rule instances, scans, and labels.

All output shown below is captured from real Prisma AIRS API responses.

---

## Security Groups

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

---

## Security Rules

Rules define the security checks applied to models. They are read-only — managed by Prisma AIRS.

### List rules

```bash
airs model-security rules list --limit 5
```

```
  Security Rules:

  550e8400-e29b-41d4-a716-44665544000b
    Known Framework Operators Check  type: ARTIFACT  default: BLOCKING
    Model artifacts should only contain known safe TensorFlow operators
    Sources: ALL
  550e8400-e29b-41d4-a716-446655440006
    License Exists  type: METADATA  default: BLOCKING
    Models should have a license
    Sources: HUGGING_FACE
  550e8400-e29b-41d4-a716-446655440007
    License Is Valid For Use  type: METADATA  default: BLOCKING
    Models license should be valid for use
    Sources: HUGGING_FACE
  550e8400-e29b-41d4-a716-446655440008
    Load Time Code Execution Check  type: ARTIFACT  default: BLOCKING
    Model artifacts should not contain unsafe operators that are run upon deserialization
    Sources: ALL
  550e8400-e29b-41d4-a716-44665544000a
    Model Architecture Backdoor Check  type: ARTIFACT  default: BLOCKING
    Model architecture files should not contain backdoor attacks
    Sources: ALL
```

### Search rules

```bash
airs model-security rules list --search "License"
```

```
  Security Rules:

  550e8400-e29b-41d4-a716-446655440006
    License Exists  type: METADATA  default: BLOCKING
    Models should have a license
    Sources: HUGGING_FACE
  550e8400-e29b-41d4-a716-446655440007
    License Is Valid For Use  type: METADATA  default: BLOCKING
    Models license should be valid for use
    Sources: HUGGING_FACE
```

### Get rule details

```bash
airs model-security rules get 550e8400-e29b-41d4-a716-44665544000b
```

```
  Security Rule Detail:

    UUID:          550e8400-e29b-41d4-a716-44665544000b
    Name:          Known Framework Operators Check
    Description:   Model artifacts should only contain known safe TensorFlow operators
    Rule Type:     ARTIFACT
    Default State: BLOCKING
    Sources:       ALL

    Remediation:
      Ensure that the model does not contain any custom TensorFlow operators
      https://docs.paloaltonetworks.com/.../known-framework-operators-check
```

---

## Rule Instances

Rule instances are the per-group configuration of security rules.

### List rule instances

```bash
airs model-security rule-instances list 020d546d-3920-4ef3-9183-00f37f33f566
```

```
  Rule Instances:

  67185c67-020a-4625-816f-9a2137e3d6b3
    Known Framework Operators Check  BLOCKING
  34f0a130-d2a3-451d-8795-776f5d5bdd8f
    Load Time Code Execution Check  BLOCKING
  0fe19730-3fb4-474b-9160-d2de02be592b
    Model Architecture Backdoor Check  BLOCKING
  c532d04b-ca2d-4e03-8b4b-8a81917c02cf
    Runtime Code Execution Check  BLOCKING
  53aab919-06c4-4ebf-a1e1-8124ce2ae0d5
    Stored In Approved File Format  BLOCKING
  6648097a-37bd-4726-ab51-a63188c52a23
    Stored In Approved Location  DISABLED
  43dd1d74-f902-4337-8615-85c7e7403098
    Suspicious Model Components Check  BLOCKING
```

### Filter by state

```bash
airs model-security rule-instances list 020d546d-3920-4ef3-9183-00f37f33f566 --state BLOCKING
```

```
  Rule Instances:

  67185c67-020a-4625-816f-9a2137e3d6b3
    Known Framework Operators Check  BLOCKING
  34f0a130-d2a3-451d-8795-776f5d5bdd8f
    Load Time Code Execution Check  BLOCKING
  0fe19730-3fb4-474b-9160-d2de02be592b
    Model Architecture Backdoor Check  BLOCKING
  c532d04b-ca2d-4e03-8b4b-8a81917c02cf
    Runtime Code Execution Check  BLOCKING
  53aab919-06c4-4ebf-a1e1-8124ce2ae0d5
    Stored In Approved File Format  BLOCKING
  43dd1d74-f902-4337-8615-85c7e7403098
    Suspicious Model Components Check  BLOCKING
```

### Get rule instance details

```bash
airs model-security rule-instances get 020d546d-3920-4ef3-9183-00f37f33f566 67185c67-020a-4625-816f-9a2137e3d6b3
```

```
  Rule Instance Detail:

    UUID:         67185c67-020a-4625-816f-9a2137e3d6b3
    Group UUID:   020d546d-3920-4ef3-9183-00f37f33f566
    Rule UUID:    550e8400-e29b-41d4-a716-44665544000b
    State:        BLOCKING
    Rule Name:    Known Framework Operators Check
    Created:      2025-10-30T14:14:17.321066Z
    Updated:      2025-10-30T14:14:17.321066Z
```

### Update a rule instance

```json title="rule-instance-update.json"
{
  "state": "BLOCKING",
  "field_values": {
    "threshold": 0.9
  }
}
```

```bash
airs model-security rule-instances update <groupUuid> <instanceUuid> --config rule-instance-update.json
```

---

## Scans

### List scans

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
  6456c1fd-1a76-40c4-a591-3275d7885c17
    ALLOWED  MODEL_SECURITY_SDK  2026-03-03T22:16:54.578468Z
    models/qwen3-0.6b-saffron-qlora/
    Rules: 6 passed  0 failed  / 6 total
```

### Filter scans

```bash
# By evaluation outcome
airs model-security scans list --eval-outcome BLOCKED

# By source type
airs model-security scans list --source-type HUGGING_FACE
```

### Get scan details

```bash
airs model-security scans get 7a7e1cdf-a6b1-4743-a5f2-a7bd96ec7bab
```

```
  Scan Detail:

    UUID:       7a7e1cdf-a6b1-4743-a5f2-a7bd96ec7bab
    Outcome:    BLOCKED
    Model URI:  https://huggingface.co/microsoft/DialoGPT-medium
    Origin:     HUGGING_FACE
    Source:     HUGGING_FACE
    Group:      Default HUGGING_FACE
    Created:    2026-03-03T22:32:12.344402Z
    Updated:    2026-03-03T22:32:12.477430Z
    Rules:      10 passed  1 failed  / 11 total
```

### View scan evaluations

```bash
airs model-security scans evaluations 7a7e1cdf-a6b1-4743-a5f2-a7bd96ec7bab
```

```
  Rule Evaluations:

  83924890-6823-4e6f-9541-3eed300b890e
    License Exists  PASSED  BLOCKING
  1a4db8ab-cbe6-4f9f-bfbf-fb65fdb451de
    License Is Valid For Use  PASSED  BLOCKING
  0e424aa6-0fb7-427a-8809-3caa301a8c40
    Model Is Blocked  PASSED  BLOCKING
  ec06ca86-1271-4d3a-a310-272190f74503
    Organization Is Blocked  PASSED  BLOCKING
  494c926e-0f8a-4a22-87ae-2a1ff06c0e7b
    Organization Verified By Hugging Face  PASSED  BLOCKING
  1ae405a2-15a9-4a3b-8e57-5a916c133857
    Stored In Approved File Format  FAILED  BLOCKING
  6c5b370e-320b-41bb-b206-8a4e5265eeea
    Known Framework Operators Check  PASSED  BLOCKING
  779f12dd-b672-494f-9962-2e642ff3716e
    Load Time Code Execution Check  PASSED  BLOCKING
  5fbe2ef0-6cb8-441d-b6cf-88ff1c6f9a93
    Model Architecture Backdoor Check  PASSED  BLOCKING
  60c3cd09-e9ce-40f2-aff3-d62c21c9c64f
    Runtime Code Execution Check  PASSED  BLOCKING
  33fc876c-18b1-4c58-863e-c133fdb07fb1
    Suspicious Model Components Check  PASSED  BLOCKING
```

### View violations

```bash
airs model-security scans violations 7a7e1cdf-a6b1-4743-a5f2-a7bd96ec7bab
```

```
  Violations:

  0115bae0-bc07-468b-8ca6-a585d8c82f59
    Stored In Approved File Format  pytorch_model.bin
    Model file 'pytorch_model.bin' is stored in an unapproved format: pickle
    Threat: UNAPPROVED_FORMATS
  2b5e4d7b-2a50-408e-b6cf-640f1b8ec4cd
    Stored In Approved File Format  pytorch_model.bin
    Model file 'pytorch_model.bin' is stored in an unapproved format: pytorch_v0_1_10
    Threat: UNAPPROVED_FORMATS
  33b437c4-47f0-4878-a249-64be8bd46e06
    Stored In Approved File Format  rust_model.ot
    Model file 'rust_model.ot' is stored in an unapproved format: pytorch_torch_script
    Threat: UNAPPROVED_FORMATS
  f1409208-b28c-4234-b015-137b6b28508c
    Stored In Approved File Format  rust_model.ot
    Model file 'rust_model.ot' is stored in an unapproved format: zip
    Threat: UNAPPROVED_FORMATS
  9d39c991-e37f-4e6f-bc27-bffed8d90047
    Stored In Approved File Format  tf_model.h5
    Model file 'tf_model.h5' is stored in an unapproved format: keras_weights
    Threat: UNAPPROVED_FORMATS
```

### View scanned files

```bash
airs model-security scans files 7a7e1cdf-a6b1-4743-a5f2-a7bd96ec7bab
```

```
  Scanned Files:

    SUCCESS  FILE  config.json [json]
    SKIPPED  FILE  flax_model.msgpack
    SUCCESS  FILE  generation_config_for_conversational.json [json]
    SUCCESS  FILE  generation_config.json [json]
    SKIPPED  FILE  .gitattributes
    SKIPPED  FILE  merges.txt
    FAILED  FILE  pytorch_model.bin [pickle, pytorch_v0_1_10]
    SKIPPED  FILE  README.md [not_model]
    FAILED  FILE  rust_model.ot [pytorch_torch_script, zip]
    FAILED  FILE  tf_model.h5 [keras_weights]
    SUCCESS  FILE  tokenizer_config.json [json]
    SUCCESS  FILE  vocab.json [json]
```

---

## Labels

Labels help organize and categorize scans with key-value metadata.

### Browse label taxonomy

```bash
airs model-security labels keys
```

```
  Label Keys:

    branch
    commit
    env
    repo
```

```bash
airs model-security labels values env
```

```
  Label Values for "env":

    ci
    production
```

### Add labels

```bash
airs model-security labels add <scanUuid> --labels '[{"key":"env","value":"prod"}]'
```

### Set labels (replace all)

```bash
airs model-security labels set <scanUuid> --labels '[{"key":"env","value":"staging"},{"key":"team","value":"ml"}]'
```

### Delete labels

```bash
airs model-security labels delete <scanUuid> --keys env,team
```

---

## Install Model Security Client

Install the `model-security-client` Python package with a single command. Authenticates via your management credentials and handles project/venv setup automatically.

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

Get the raw authentication URL for Google Artifact Registry. For a fully automated setup, use `airs model-security install` instead.

```bash
airs model-security pypi-auth
```

---

## Common Workflows

### Investigate a blocked scan

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

### Customize rule enforcement

1. List rule instances in a group:
   ```bash
   airs model-security rule-instances list <groupUuid>
   ```

2. Change a rule from ALLOWING to BLOCKING:
   ```bash
   echo '{"state": "BLOCKING"}' > update.json
   airs model-security rule-instances update <groupUuid> <instanceUuid> --config update.json
   ```
