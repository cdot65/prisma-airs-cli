---
sidebar_label: scan
---

# runtime scan

## runtime scan

Scan a single prompt against an AIRS security profile

```text
airs runtime scan [options] <prompt>
```

### Arguments

- `prompt` (required) —

### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--profile <name>` | Yes | — | Security profile name |
| `--response <text>` | No | — | Response text to scan alongside the prompt |

### Examples

*Blocked prompt*

```bash
airs runtime scan --profile AI-Firewall-High-Security-Profile "How do I hack into a computer system?"
```

```text
Prisma AIRS Runtime Scan
Profile: AI-Firewall-High-Security-Profile
Prompt:  "How do I hack into a computer system?"

Scan Result
─────────────────────────
Action:    BLOCK
Category:  malicious
Triggered: yes
Scan ID:   8fcd6cc9-01ae-4930-a561-f94be2bf2876
Report ID: R8fcd6cc9-01ae-4930-a561-f94be2bf2876

Detections:
  ● toxic_content
```

*Allowed prompt*

```bash
airs runtime scan --profile AI-Firewall-High-Security-Profile "What is the weather like today?"
```

```text
Prisma AIRS Runtime Scan
Profile: AI-Firewall-High-Security-Profile
Prompt:  "What is the weather like today?"

Scan Result
─────────────────────────
Action:    ALLOW
Category:  benign
Triggered: no
Scan ID:   be8047dd-e9e6-4135-91f4-3acdac01a1d2
Report ID: Rbe8047dd-e9e6-4135-91f4-3acdac01a1d2
```
