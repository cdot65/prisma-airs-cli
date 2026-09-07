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

Captured **2026-09-07T12:10:57.358Z** from actual CLI **4.4.0** execution with SDK **0.25.0** against AI Gateway.
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
  "created": 1788783044,
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

On September 6, 2026 the built CLI passed 8/8 live inference checks using a temporary
SCM-created key, the designated dev workspace, native fetch, and the requested OpenAI models.
The key was removed afterward and the user's config remained unchanged.

The latest capture above comes from independently installed **CLI 4.4.0 candidate** with published
SDK **0.25.0**, not a development link. All seven payload files and 19 library exports match the
npm-built archive. The manifest and frozen lockfile pin the SDK exactly; SDK publication preceded
the CLI dependency update. CLI registry publication and a user-prefix upgrade remain pending.

All **1,268 CLI tests** and **14 release-policy/diagnostic checks** pass. The installed candidate passes
**103/103** [grouped-filter checks](./telemetry.md#verified-grouped-filter-output), **54/54**
[chart-filter checks](./telemetry.md#verified-chart-filter-output), **3/3**
[empty latency JSON/YAML checks](./telemetry.md#verified-empty-window-output), and **11/11** native
DLP checks across all five formats. Cross-service reads and the benign scan pass **12/12** at
**2026-09-07T12:12:38.879Z**. Independent retirement confirms **35/35** historical release keys absent
at **2026-09-07T12:13:37.774Z**. No runtime key is persisted and configuration remains unchanged.

Earlier 4.3.0/4.3.1 installation failures and registry processing delays remain in private history;
those releases retain their own timestamps in the SDK's published-package guide. The first 4.4.0
grouped candidate run passed 102/103; the complete subsequent run passes 103/103 without changing
validation, retries or infrastructure. Full Gateway coverage remains 138/242 (57.02%), not 99%.
The earlier CLI 4.2.x registry verification remains historical evidence in the SDK's published-package guide.
No realtime CLI command is added; the SDK's experimental realtime transport reaches HTTP 101
but the prescribed model rejects provider-session creation with `invalid_model`.
Known AI Gateway gaps remain explicit in the [release notes](../../about/release-notes.md).
The SDK's `scripts/e2e-cli-inference.ts --writes` reproduces the live check from read-only
SCM credentials without persisting a runtime key.
