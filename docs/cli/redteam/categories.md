# redteam categories

## redteam categories

List available attack categories

```text
airs redteam categories [options]
```

### Examples

*List attack categories. IDs in parens are what `--categories` wants on a STATIC scan,
e.g. `--categories '{"SECURITY":["JAILBREAK","PROMPT_INJECTION"]}'`.
*

```bash
airs redteam categories
```

```text
Attack Categories:

Security (SECURITY) — Select categories for adversarial testing of security vulnerabilities and potential exploits.
  • Adversarial Suffix (ADVERSARIAL_SUFFIX) — Adversarial suffix attacks
  • Jailbreak (JAILBREAK) — Jailbreak attempts
  • Prompt Injection (PROMPT_INJECTION) — Direct prompt injection attacks
  • Remote Code Execution (REMOTE_CODE_EXECUTION) — Remote code execution attempts
  • System Prompt leak (SYSTEM_PROMPT_LEAK) — System prompt extraction
  ...

Safety (SAFETY) — Select categories for testing harmful or toxic content and ethical misuse scenarios.
  • Bias (BIAS) — Bias-related content
  • CBRN (CBRN) — Chemical, Biological, Radiological, Nuclear content
  • Hate / Toxic / Abuse (HATE_TOXIC_ABUSE) — Hate speech, toxic, or abusive content
  ...

Brand Reputation (BRAND_REPUTATION) — Select categories for testing off-brand content.
  • Competitor Endorsements (COMPETITOR_ENDORSEMENTS) — Content endorsing competitor brands
  ...

Compliance (COMPLIANCE) — Select framework to understand compliance across security and safety standards.
  • OWASP Top 10 for LLMs 2025 (OWASP_TOP_10_LLM_2025) — Open Web Application Security Project 2025 Edition
  • MITRE ATLAS (MITRE_ATLAS) — MITRE Adversarial Tactics, Techniques, and Common Knowledge
  • NIST AI-RMF (NIST_AI_RMF) — National Institute of Standards and Technology Cybersecurity Framework
  ...
```
