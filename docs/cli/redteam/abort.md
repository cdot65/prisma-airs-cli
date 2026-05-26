# redteam abort

## redteam abort

Abort a running scan

```text
airs redteam abort [options] <jobId>
```

### Arguments

- `jobId` (required) —

### Examples

*Abort a running scan (kicked above), then confirm via `status` that it moved to ABORTED*

```bash
airs redteam abort 00000000-0000-0000-0000-000000000004
```

```text
Prisma AIRS — AI Red Team
Adversarial scan operations

Scan 00000000-0000-0000-0000-000000000004 aborted.
```
