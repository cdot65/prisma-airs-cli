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

Captured **2026-09-07T02:01:20.260Z** from the built CLI against the freshly packed SDK candidate.
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
  "created": 1788746472,
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

Streaming chat/Responses, both embedding encodings, invalid-input exit codes and temporary-key cleanup passed in the same 8/8 suite. This is candidate evidence, not a published release.

This run used an opt-in, process-only DNS accommodation in the test workspace: fully qualified service lookups and the gateway's existing LAN ingress address, independently verified through its configured secondary DNS resolver. The original HTTPS hostname/SNI, certificate verification and gateway authentication were preserved. No infrastructure settings changed. The public WAN path timed out from this workspace and is not certified by these results.

## Validation and release dependency

On September 6, 2026 the built CLI passed 8/8 live inference checks using a temporary
SCM-created key, the designated dev workspace, native fetch, and the requested OpenAI models.
The key was removed afterward and the user's config remained unchanged.

The capture above is pre-release evidence from the locally packed SDK candidate. CLI 4.2.0 targets
the published SDK 0.21.0 with an exact dependency pin; SDK publication precedes CLI publication.
Known AI Gateway gaps remain explicit in the [release notes](../../about/release-notes.md).
The SDK's `scripts/e2e-cli-inference.ts --writes` reproduces the live check from read-only
SCM credentials without persisting a runtime key.
