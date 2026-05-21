# Finding: SVG bypasses DLP inspection

!!! danger "Summary"
    On the tested AIRS profile, **wrapping sensitive data in a valid SVG document causes the
    DLP engine to be skipped entirely.** The exact same data, sent as plaintext, is blocked.
    Injection / toxic-content / malicious-code detectors still run on SVG content — only DLP
    is bypassed.

| | |
| --- | --- |
| **Product** | Palo Alto Prisma AIRS (AI Runtime Security), AI-Runtime-API |
| **Tested via** | `airs` CLI v2.7.0 (`airs runtime scan`) |
| **Profile** | `AI Gateway - Dev - Strict` (DLP policy `data-leak-detection.action = block`) |
| **Date** | 2026-05-21 |
| **Severity** | High — undetected exfiltration of sensitive data |
| **Data used** | Synthetic only (reserved test SSN, Visa test PAN, etc.) |

## Impact

Any sensitive data placed inside a namespaced `<svg>` element passes DLP undetected — whether
the data is in markup (`<metadata>`, `<desc>`, comments), hidden `<text>`, or **fully visible
rendered text**. This is a clean DLP exfiltration channel for a data-loss policy that otherwise
blocks the same content. (Prompt-injection and script payloads inside SVG are still caught, so
the gap is specific to DLP.)

## Evidence

Identical PII, two carriers, same profile:

| Case | Input | Result | Scan ID |
| --- | --- | --- | --- |
| Control | `SSN 078-05-1120 Visa 4111 1111 1111 1111` (plaintext) | **BLOCK** · `dlp` | `3c6b14b0-6708-4cb8-b50c-3da047b14b42` |
| Bypass | `<svg xmlns="http://www.w3.org/2000/svg">SSN 078-05-1120 Visa 4111 1111 1111 1111</svg>` | **ALLOW** · benign | `0bd75bae-cb7f-4b83-8f7c-ed1d94ec3071` |

Profile DLP is confirmed enabled: `policy → ai-security-profiles[0] → model-configuration →
data-protection → data-leak-detection → action: "block"`.

## Reproduction

```bash
# blocked (dlp)
airs runtime scan --profile "AI Gateway - Dev - Strict" \
  "SSN 078-05-1120 Visa 4111 1111 1111 1111"

# allowed (bypass) — identical data wrapped in a namespaced SVG
airs runtime scan --profile "AI Gateway - Dev - Strict" \
  '<svg xmlns="http://www.w3.org/2000/svg">SSN 078-05-1120 Visa 4111 1111 1111 1111</svg>'
```

## Root cause — isolated by probe

The trigger is **SVG document classification** (an `<svg>` root *with* the SVG namespace),
not XML tags in general, and not the namespace string alone. DLP is then skipped — even for
visibly-rendered PII (so it is not an OCR-only path; DLP is simply not applied).

| Probe | DLP fires? |
| --- | --- |
| plaintext / pipe-delimited markers | ✅ block |
| markers in bare `<text>…</text>` tags (no `<svg>`) | ✅ block |
| generic XML `<root>…</root>` | ✅ block |
| `<svg>…</svg>` **without** namespace | ✅ block |
| namespace string alone (no `<svg` tag) | ✅ block |
| **`<svg xmlns="http://www.w3.org/2000/svg">…</svg>`** | ❌ **allow** |
| namespaced SVG with PII as large **visible** text | ❌ **allow** |

## Cross-detector behavior (full SVG corpus)

Same result whether submitted as raw SVG text or base64 — confirming AIRS *does* read SVG
content; only the DLP detector is gated out:

| SVG file | Outcome |
| --- | --- |
| `svg_mal_1_dlp` (sensitive data only) | **ALLOW** — bypass |
| `svg_mal_2_prompt_injection` | BLOCK (`injection`) |
| `svg_mal_3_system_prompt` | BLOCK (`injection`) |
| `svg_mal_4_exfil_injection` | BLOCK (`injection`, `toxic_content`) |
| `svg_mal_5_script_xss` | BLOCK (`injection`, `toxic_content`) |
| `svg_benign_1/2` | ALLOW (correct) |

## Recommendation

SVG is XML/text, not a raster image. AIRS should apply DLP text inspection to SVG content
(extract and scan text nodes / attributes / `<metadata>` / `<desc>` / comments), or route SVG
through the same text DLP path used for plaintext — rather than classifying it as an image and
skipping DLP.
