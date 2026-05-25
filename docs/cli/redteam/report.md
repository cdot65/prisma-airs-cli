# redteam report

## redteam report

View scan report

```text
airs redteam report [options] <jobId>
```

### Arguments

- `jobId` (required) —

### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--attacks` | No | `false` | Include attack list |
| `--severity <level>` | No | — | Filter attacks by severity |
| `--limit <n>` | No | `20` | Max attacks to show |

### Examples

*Full scan report (STATIC scan — score, severity breakdown, categories, summary, recommendations)*

```bash
airs redteam report 00000000-0000-0000-0000-000000000001
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


Static Scan Report:
  Score: 2.76
  ASR:   3.2%

Severity Breakdown:
  LOW        32 bypassed  616 blocked
  MEDIUM     45 bypassed  1431 blocked
  HIGH       57 bypassed  1797 blocked
  CRITICAL   6 bypassed  378 blocked

Categories:
  Adversarial Suffix             ASR: 4.3%  (6/138)
  Evasion                        ASR: 1.4%  (8/564)
  Indirect Prompt Injection      ASR: 0.0%  (0/60)
  Jailbreak                      ASR: 4.5%  (47/1044)
  Prompt Injection               ASR: 4.2%  (27/648)
  Remote Code Execution          ASR: 2.8%  (1/36)
  System Prompt leak             ASR: 0.4%  (1/264)
  Tool Leak                      ASR: 0.9%  (1/114)
  Malware Generation             ASR: 6.9%  (5/72)

Summary:
  In this Attack Library Scan, the target was tested for Brand, Compliance, Safety and Security risks.

### Summary
The application has low risk with an overall Risk Score of 2.76/100.

### Recommendations

**1. Enforcing Least Privilege Access Control**:
Grant your AI application only the minimum permissions necessary to perform its intended functions. Limit access to sensitive resources, APIs, and data.

**2. Paraphrasing**:
Implement paraphrasing techniques to disrupt potential manipulation attempts by restructuring and reinterpreting input data.

**3. Runtime Execution Constraints**:
Set time limits and resource consumption thresholds for code execution.
```

*Attack list filtered by severity*

```bash
airs redteam report 00000000-0000-0000-0000-000000000001 --attacks --severity CRITICAL --limit 3
```

```text
... (report summary as above) ...

Attacks:

  CRITICAL   BLOCKED  undefined [SECURITY]
  CRITICAL   BLOCKED  undefined [SECURITY]
  CRITICAL   BLOCKED  undefined [SECURITY]
```
