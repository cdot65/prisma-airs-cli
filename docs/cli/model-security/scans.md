# model-security scans

### model-security scans list

List model security scans

```text
airs model-security scans list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--eval-outcome <outcome>` | No | — | Filter by eval outcome |
| `--source-type <type>` | No | — | Filter by source type |
| `--scan-origin <origin>` | No | — | Filter by scan origin |
| `--search <query>` | No | — | Search scans |
| `--limit <n>` | No | `20` | Max results |
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

#### Examples

*List model security scans*

```bash
airs model-security scans list
```

```text
Model Security Scans:

7a7e1cdf-a6b1-4743-a5f2-a7bd96ec7bab
  BLOCKED  HUGGING_FACE  2026-03-03T22:32:12.344402Z
  https://huggingface.co/microsoft/DialoGPT-medium
  Rules: 10 passed  1 failed  / 11 total
ee71b4da-64ce-4d6c-96fb-2bced1154a06
  ALLOWED  MODEL_SECURITY_SDK  2026-03-03T22:21:44.130386Z
  /Users/cdot/models/qwen3-0.6b-saffron-merged
  Rules: 6 passed  0 failed  / 6 total
```

---

### model-security scans get

Get scan details

```text
airs model-security scans get [options] <uuid>
```

#### Arguments

- `uuid` (required) —

#### Examples

*Pretty output (default; no --output flag on this command). Re-run after scan completes to see final Outcome + rule counts.*

```bash
airs model-security scans get 00000000-0000-0000-0000-000000000002
```

```text
Prisma AIRS — Model Security
ML model supply chain security


Scan Detail:

  UUID:       00000000-0000-0000-0000-000000000002
  Outcome:    BLOCKED
  Model URI:  https://huggingface.co/hf-internal-testing/tiny-random-bert
  Origin:     MODEL_SECURITY_SDK
  Source:     HUGGING_FACE
  Group:      group-docs-test
  Created:    2026-05-26T10:22:18.284287Z
  Updated:    2026-05-26T10:22:18.346345Z
  Rules:      7 passed  4 failed  / 11 total
```

---

### model-security scans create

Create a model security scan

```text
airs model-security scans create [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--config <path>` | Yes | — | JSON file with scan configuration |

#### Examples

*Submit a Hugging Face model URI to be scanned by the AIRS backend.
For HF URIs you provide model_uri + security_group_uuid + scan_origin only;
the backend fetches and scans the model and the resulting outcome is
available via `scans get` shortly after. See
docs/cli/examples/model-security/scan-create.json.
*

```bash
airs model-security scans create --config docs/cli/examples/model-security/scan-create.json
```

```text
Prisma AIRS — Model Security
ML model supply chain security

Scan created: 00000000-0000-0000-0000-000000000002


Scan Detail:

  UUID:       00000000-0000-0000-0000-000000000002
  Outcome:    PENDING
  Model URI:  https://huggingface.co/hf-internal-testing/tiny-random-bert
  Origin:     MODEL_SECURITY_SDK
  Source:     HUGGING_FACE
  Group:      group-docs-test
  Created:    2026-05-26T10:22:18.284287Z
  Updated:    2026-05-26T10:22:18.284287Z
```

---

### model-security scans evaluations

List rule evaluations for a scan

```text
airs model-security scans evaluations [options] <scanUuid>
```

#### Arguments

- `scanUuid` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--limit <n>` | No | `20` | Max results |

#### Examples

*List rule evaluations for a scan (one row per rule the scan was checked against)*

```bash
airs model-security scans evaluations 00000000-0000-0000-0000-000000000002
```

```text
Prisma AIRS — Model Security
ML model supply chain security


Rule Evaluations:

00000000-0000-0000-0000-000000000004
  License Exists  FAILED  BLOCKING
00000000-0000-0000-0000-000000000005
  License Is Valid For Use  FAILED  BLOCKING
00000000-0000-0000-0000-000000000006
  Model Is Blocked  PASSED  BLOCKING
00000000-0000-0000-0000-000000000007
  Organization Is Blocked  PASSED  BLOCKING
00000000-0000-0000-0000-000000000008
  Organization Verified By Hugging Face  FAILED  BLOCKING
00000000-0000-0000-0000-000000000009
  Stored In Approved File Format  FAILED  BLOCKING
00000000-0000-0000-0000-00000000000a
  Known Framework Operators Check  PASSED  ALLOWING
00000000-0000-0000-0000-00000000000b
  Load Time Code Execution Check  PASSED  BLOCKING
00000000-0000-0000-0000-00000000000c
  Model Architecture Backdoor Check  PASSED  BLOCKING
00000000-0000-0000-0000-00000000000d
  Runtime Code Execution Check  PASSED  BLOCKING
00000000-0000-0000-0000-00000000000e
  Suspicious Model Components Check  PASSED  BLOCKING
```

---

### model-security scans evaluation

Get evaluation details

```text
airs model-security scans evaluation [options] <uuid>
```

#### Arguments

- `uuid` (required) —

#### Examples

*Get details for a single rule evaluation (UUID from `scans evaluations`)*

```bash
airs model-security scans evaluation 00000000-0000-0000-0000-000000000004
```

```text
Prisma AIRS — Model Security
ML model supply chain security


Evaluation Detail:

  UUID:           00000000-0000-0000-0000-000000000004
  Rule:           License Exists
  Description:    Models should have a license
  Instance UUID:  00000000-0000-0000-0000-00000000000f
  Instance State: BLOCKING
  Result:         FAILED
  Violations:     1
```

---

### model-security scans violations

List violations for a scan

```text
airs model-security scans violations [options] <scanUuid>
```

#### Arguments

- `scanUuid` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--limit <n>` | No | `20` | Max results |

#### Examples

*List violations for a scan*

```bash
airs model-security scans violations 00000000-0000-0000-0000-000000000002
```

```text
Prisma AIRS — Model Security
ML model supply chain security


Violations:

00000000-0000-0000-0000-000000000010
  Organization Verified By Hugging Face  
  The org/model pair (**hf-internal-testing/tiny-random-bert**) is denied since it has not been explicitly approved
  Threat: 
00000000-0000-0000-0000-000000000011
  License Exists  
  input data has a null license
  Threat: 
00000000-0000-0000-0000-000000000012
  Stored In Approved File Format  pytorch_model.bin
  Model file `pytorch_model.bin` is stored in an unapproved format: **pytorch_v1_13**
  Threat: UNAPPROVED_FORMATS
00000000-0000-0000-0000-000000000013
  Stored In Approved File Format  pytorch_model.bin
  Model file `pytorch_model.bin` is stored in an unapproved format: **zip**
  Threat: UNAPPROVED_FORMATS
00000000-0000-0000-0000-000000000014
  Stored In Approved File Format  tf_model.h5
  Model file `tf_model.h5` is stored in an unapproved format: **keras_weights**
  Threat: UNAPPROVED_FORMATS
00000000-0000-0000-0000-000000000015
  Stored In Approved File Format  vocab.txt
  Model file `vocab.txt` is stored in an unapproved format: **openvino_bin**
  Threat: UNAPPROVED_FORMATS
00000000-0000-0000-0000-000000000016
  Stored In Approved File Format  onnx/model.onnx
  Model file `onnx/model.onnx` is stored in an unapproved format: **onnx**
  Threat: UNAPPROVED_FORMATS
00000000-0000-0000-0000-000000000017
  License Is Valid For Use  
  Invalid license: **null**
  Threat: 
```

---

### model-security scans violation

Get violation details

```text
airs model-security scans violation [options] <uuid>
```

#### Arguments

- `uuid` (required) —

#### Examples

*Get details for a single violation (UUID from `scans violations`)*

```bash
airs model-security scans violation 00000000-0000-0000-0000-000000000010
```

```text
Prisma AIRS — Model Security
ML model supply chain security


Violation Detail:

  UUID:        00000000-0000-0000-0000-000000000010
  Rule:        Organization Verified By Hugging Face
  Description: Organization that produced the model should be verified by Hugging Face
  State:       BLOCKING
  File:        
  Threat:      
  Detail:      The org/model pair (**hf-internal-testing/tiny-random-bert**) is denied since it has not been explicitly approved
```

---

### model-security scans files

List scanned files

```text
airs model-security scans files [options] <scanUuid>
```

#### Arguments

- `scanUuid` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--type <type>` | No | — | Filter by file type |
| `--result <result>` | No | — | Filter by result |
| `--limit <n>` | No | `20` | Max results |

#### Examples

*List files the scanner processed (FAILED rows are the ones with violations)*

```bash
airs model-security scans files 00000000-0000-0000-0000-000000000002
```

```text
Prisma AIRS — Model Security
ML model supply chain security


Scanned Files:

  SUCCESS  FILE  config.json [json]
  SKIPPED  FILE  .gitattributes
  SUCCESS  FILE  model.safetensors [safetensors]
  SUCCESS  DIRECTORY  onnx
  FAILED  FILE  pytorch_model.bin [zip, pytorch_v1_13]
  SUCCESS  FILE  special_tokens_map.json [json]
  FAILED  FILE  tf_model.h5 [keras_weights]
  SUCCESS  FILE  tokenizer_config.json [json]
  SUCCESS  FILE  tokenizer.json [json]
  FAILED  FILE  vocab.txt [openvino_bin]
```
