---
sidebar_label: runtime inference
---

# Runtime inference

`airs aigateway inference` uses your deployed gateway endpoint and runtime API key, independently
of SCM management OAuth. Configure `PANW_AI_GW_INFERENCE_ENDPOINT` (including `/v1`) and
`PANW_AI_GW_INFERENCE_API_KEY`, or the corresponding `aiGwInferenceEndpoint` and
`aiGwInferenceApiKey` config keys. Commands read config; they do not write or rotate credentials.
There is deliberately no API-key command-line flag. Never put a real key in shell history.

```bash
airs aigateway inference chat 'Reply with READY.' \
  --model @openai/gpt-5.6-terra --max-tokens 128 --output json

airs aigateway inference chat 'Reply with READY.' \
  --model @openai/gpt-5.6-terra --max-tokens 128 --stream --output json

airs aigateway inference responses 'Reply with READY.' \
  --model @openai/gpt-5.6-terra --max-tokens 128 --stream

airs aigateway inference embeddings 'Readiness check' \
  --model @openai/text-embedding-3-small --dimensions 32 --encoding float --output json
```

`--stream --output json` emits **JSONL**, not one JSON document: each line is a validated event.
Pretty streaming prints text. YAML streaming is rejected before making a request. Non-streaming
JSON and YAML preserve the SDK response. Embeddings preserve float or base64 representations.
Stdout contains data only. Request errors exit 2; network/API/failed or incomplete Responses
generations exit 1. Closed output pipes stop generation cleanly; SIGINT/SIGTERM abort network
work, release the stream and exit nonzero. Output writes respect pipe backpressure.

`--file request.json` accepts the full typed JSON request, including embedding batches or
structured chat messages. It cannot be combined with a prompt argument. Explicit command flags
override the file. A file's model overrides config defaults. Model defaults use
`PANW_AI_GW_INFERENCE_MODEL` / `aiGwInferenceModel` and
`PANW_AI_GW_EMBEDDING_MODEL` / `aiGwEmbeddingModel`. No model is selected automatically.

The default timeout is 60,000 ms and includes stream consumption; override it with `--timeout`.
Inference retries are disabled to avoid duplicate billable calls. Ordinary Responses prompts use
`store: false`; use a request file to opt into storage. Runtime debug logs record status and
redacted headers but omit request/response bodies and never consume a stream ahead of the SDK.

## Latest verified example output

Captured **2026-09-07T12:29:23.185Z** from actual CLI **4.4.0** execution with SDK **0.25.0** against AI Gateway.
Actual built CLI JSON stdout from the passing live suite; response identifiers are redacted. No runtime key or configuration value is retained.

The chat command above returned:

```json
{
  "id": "<response-id>",
  "choices": [
    {
      "finish_reason": "stop",
      "index": 0,
      "message": {
        "content": "READY",
        "role": "assistant",
        "refusal": null,
        "annotations": []
      }
    }
  ],
  "created": 1788784154,
  "model": "gpt-5.6-terra",
  "system_fingerprint": null,
  "object": "chat.completion",
  "usage": {
    "completion_tokens": 4,
    "prompt_tokens": 10,
    "total_tokens": 14,
    "completion_tokens_details": {
      "reasoning_tokens": 0,
      "accepted_prediction_tokens": 0,
      "rejected_prediction_tokens": 0,
      "audio_tokens": 0
    },
    "prompt_tokens_details": {
      "cached_tokens": 0,
      "cache_write_tokens": 0,
      "audio_tokens": 0
    }
  },
  "service_tier": "default"
}
```

Streaming chat/Responses, both embedding encodings, invalid-input exit codes and temporary-key cleanup passed in the same 8/8 suite. This verifies those workflows, not the missing or failed AI Gateway operations.

This run used an opt-in, process-only DNS accommodation in the test workspace: fully qualified service lookups and the gateway's existing LAN ingress address, independently verified through its configured secondary DNS resolver. The original HTTPS hostname/SNI, certificate verification and gateway authentication were preserved. No infrastructure settings changed. The public WAN path timed out from this workspace and is not certified by these results.

## Validation and release dependency {#validated-candidate-and-release-ordering}

CLI **4.4.0** is published, independently installed from npm and upgraded in the user prefix. It pins the already-published SDK **0.25.0** exactly; all seven payload files and 19 exports match the tested npm archive. Registry integrity agrees at **2026-09-07T12:28:22.542Z**: **123,664 bytes**, SHA-256 **`f53c31400945a4004d15c5a7368f97dae6101c8ccbb97d71f5821dbf86dcb7c1`**.

- Registry grouped filters: **103/103**, **2026-09-07T12:32:16.444Z**.
- Registry chart contracts: **54/54**, **2026-09-07T12:35:21.598Z**.
- Registry inference: **8/8**, **2026-09-07T12:29:23.185Z**.
- Registry empty-window JSON/YAML: **3/3**, **2026-09-07T12:29:09.610Z**.
- User-installed cross-service reads/benign scan: **12/12**, **2026-09-07T12:29:31.518Z**.
- Independent historical release-key retirement: **36/36**, **2026-09-07T12:30:44.576Z**.
- Native DLP: **11/11 registry** at **2026-09-07T12:26:53.881Z**, **11/11 user-installed** at **2026-09-07T12:29:11.324Z**.

All 1,268 CLI tests and 14 release-policy/diagnostic tests pass; the existing library-only coverage scope reports 95.97% lines/statements, 95.48% functions and 87.34% branches. Frozen and fresh registry production audits are clean. The [container workflow](https://github.com/cdot65/prisma-airs-cli/actions/runs/34121350201) passes 11/11 native checks on each actual architecture without runtime network or credentials, then verifies minor/latest aliases at **`sha256:6134107b600e76491a70bcbdc261337abe9d53673082c1cc1a058676c87e6b52`**. Temporary native corpora are cleaned up before success is recorded. Host checks use the disclosed process-only font configuration; container checks use packaged fonts.

Failed attempts remain in history: the first grouped candidate run passed 102/103, the first registry chart run 53/54, and the first public browser run 10/11 with a DNS-resolution failure. The complete subsequent suites pass without changing validation, SDK retries, models, credentials or infrastructure. The chart rerun is serialized after the grouped suite; the masked initial subprocess failure does not establish a root cause. Registry metadata visibility lagged successful publication; no duplicate publication was attempted.

The credential file remains unchanged and no runtime key is persisted. All 101 grouped-filter pairs and all 52 chart-filter pairs are reproduced on the CLI telemetry page. Direct Gateway coverage remains **138/242 (57.02%)**, all 22 analytics adaptations remain partial, and the full-project assessment remains **5/10**. Existing service/provider/entitlement failures, SDK documentation dependency advisories, public WAN and anonymous container-pull limits remain open. No realtime CLI command or successful provider realtime session is claimed.
