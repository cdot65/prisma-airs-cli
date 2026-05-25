# redteam status

## redteam status

Check scan status

```text
airs redteam status [options] <jobId>
```

### Arguments

- `jobId` (required) —

### Examples

*Check progress / final score for any scan (STATIC / DYNAMIC / CUSTOM)*

```bash
airs redteam status 00000000-0000-0000-0000-000000000001
```

```text
Prisma AIRS — AI Red Team
Adversarial scan operations

Scan Status:
  ID:      00000000-0000-0000-0000-000000000001
  Name:    Example Static Scan
  Type:    STATIC
  Target:  Example Target
  Status:  COMPLETED
  Progress: 1454/1464
  Score:   2.76
  ASR:     3.2%
```
