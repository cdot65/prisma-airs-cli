---
title: Rules & Instances
---

# Security Rules

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

# Rule Instances

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
