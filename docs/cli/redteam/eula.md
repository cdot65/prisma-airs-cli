# redteam eula

### redteam eula status

Check EULA acceptance status

```text
airs redteam eula status [options]
```

#### Examples

*Check whether the EULA has been accepted on this tenant*

```bash
airs redteam eula status
```

```text
Prisma AIRS — AI Red Team
Adversarial scan operations


EULA Status:

  Accepted: yes
  Accepted At: 2025-11-03T15:22:02.528000Z
  Accepted By: 00000000-0000-0000-0000-000000000002
```

---

### redteam eula content

Display EULA content

```text
airs redteam eula content [options]
```

#### Examples

*Display the EULA text (output truncated — actual document is ~500 lines)*

```bash
airs redteam eula content
```

```text
Prisma AIRS — AI Red Team
Adversarial scan operations


EULA Content:

  ## END USER LICENSE AGREEMENT

---

**THIS END USER LICENSE AGREEMENT ("Agreement") GOVERNS THE USE OF PALO ALTO
NETWORKS PRODUCTS (as that term "Product" is defined below).**

THIS IS A LEGAL AGREEMENT BETWEEN YOU (REFERRED TO HEREIN AS "CUSTOMER" or
"END USER") AND PALO ALTO NETWORKS, INC. ...

...
(full EULA text — definitions, use rights, restrictions, warranty, etc.)
...
```

---

### redteam eula accept

Accept the EULA

```text
airs redteam eula accept [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--confirm` | No | — | Skip confirmation prompt |

#### Examples

*Accept the EULA on this tenant (idempotent — re-running just re-confirms the existing acceptance)*

```bash
airs redteam eula accept
```

```text
Prisma AIRS — AI Red Team
Adversarial scan operations


EULA Status:

  Accepted: yes
  Accepted At: 2026-01-01T00:00:00.000000Z
  Accepted By: 00000000-0000-0000-0000-000000000005

EULA accepted.
```
