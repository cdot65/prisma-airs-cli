# model-security rules

### model-security rules list

List available security rules

```text
airs model-security rules list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--source-type <type>` | No | — | Filter by source type |
| `--search <query>` | No | — | Search by name or UUID |
| `--limit <n>` | No | `20` | Max results |
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

#### Examples

*List security rules*

```bash
airs model-security rules list
```

```text
Security Rules:

550e8400-e29b-41d4-a716-44665544000b
  Known Framework Operators Check  type: ARTIFACT  default: BLOCKING
  Model artifacts should only contain known safe TensorFlow operators
  Sources: ALL
550e8400-e29b-41d4-a716-446655440008
  Load Time Code Execution Check  type: ARTIFACT  default: BLOCKING
  Model artifacts should not contain unsafe operators that are run upon deserialization
  Sources: ALL
550e8400-e29b-41d4-a716-446655440009
  Suspicious Model Components Check  type: ARTIFACT  default: BLOCKING
  Model artifacts should not contain suspicious components
  Sources: ALL
```

---

### model-security rules get

Get security rule details

```text
airs model-security rules get [options] <uuid>
```

#### Arguments

- `uuid` (required) —

#### Examples

*Pretty output (default; no --output flag on this command)*

```bash
airs model-security rules get 550e8400-e29b-41d4-a716-44665544000b
```

```text
Prisma AIRS — Model Security
ML model supply chain security


Security Rule Detail:

  UUID:          550e8400-e29b-41d4-a716-44665544000b
  Name:          Known Framework Operators Check
  Description:   Model artifacts should only contain known safe TensorFlow operators
  Rule Type:     ARTIFACT
  Default State: BLOCKING
  Sources:       ALL

  Remediation:
    Ensure that the model does not contain any custom TensorFlow operators that can execute arbitrary code
    • Review the violation details to understand which operators were flagged.
    • This rule cannot allowlist individual operators - it either blocks or allows all unknown operators. If your environment regularly uses trusted custom operators, set this rule to Allowing mode in the security group that scanned this model.
    • If the operators are unknown or untrusted, do not use the model and source an alternative that uses only standard framework operators.
    https://docs.paloaltonetworks.com/ai-runtime-security/ai-model-security/model-security-to-secure-your-ai-models/understanding-ai-model-security-rules#concept-rrb_bqp_xgc
```
