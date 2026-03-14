# Troubleshooting

Something not working? Find your error below.

---

## SaaS APIs (`claude-api`, `gemini-api`)

| Error | Cause | Fix |
|-------|-------|-----|
| `401 Unauthorized` / `invalid x-api-key` | Missing or invalid API key | Verify `ANTHROPIC_API_KEY` or `GOOGLE_API_KEY` is set and correct |
| `429 Rate limit exceeded` | Too many requests | Reduce `SCAN_CONCURRENCY` or wait before retrying |
| `Could not resolve model` | Wrong model name for provider | Verify model name matches API format (see [overview](overview.md#override-the-default-model)) |
| `ENOTFOUND api.anthropic.com` | Network / DNS issue | Check internet connectivity and proxy settings |

!!! tip "Validate your API key"

    === "Anthropic"

        ```bash
        curl https://api.anthropic.com/v1/messages \
          -H "x-api-key: $ANTHROPIC_API_KEY" \
          -H "content-type: application/json" \
          -H "anthropic-version: 2023-06-01" \
          -d '{"model":"claude-sonnet-4-20250514","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}'
        ```

    === "Google AI"

        ```bash
        curl "https://generativelanguage.googleapis.com/v1beta/models?key=$GOOGLE_API_KEY"
        ```

---

## Vertex AI (`claude-vertex`, `gemini-vertex`)

| Error | Cause | Fix |
|-------|-------|-----|
| `Could not load the default credentials` | ADC not configured | Run `gcloud auth application-default login` |
| `Project not found` | Wrong project ID | Verify `GOOGLE_CLOUD_PROJECT` matches your GCP project |
| `Permission denied` (403) | Missing IAM role | Grant **Vertex AI User** (`roles/aiplatform.user`) to your account |
| `Model not found` | Wrong model name or unavailable in region | Check model name and regional availability |
| `Quota exceeded` | Project quota exhausted | Request a quota increase in the GCP console |

!!! warning "Claude on Vertex defaults to `global` region"

    If you see region-related errors with `claude-vertex`, check that `GOOGLE_CLOUD_LOCATION` is set correctly. The default is `global`, not `us-central1`.

!!! info "Refresh ADC credentials"

    ADC tokens expire. If you see authentication errors after credentials were previously working:

    ```bash
    gcloud auth application-default login
    ```

---

## Bedrock (`claude-bedrock`, `gemini-bedrock`)

| Error | Cause | Fix |
|-------|-------|-----|
| `UnrecognizedClientException` | Invalid AWS credentials | Verify `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` |
| `AccessDeniedException` | Model not enabled in Bedrock | Enable the model in the [Bedrock console](https://console.aws.amazon.com/bedrock/) under **Model access** |
| `ValidationException` | Wrong model ID format | Use full Bedrock model ID (e.g., `anthropic.claude-opus-4-6-v1`) |
| `Could not resolve credentials` | No credentials found | Set env vars or configure `~/.aws/credentials` |
| `ExpiredTokenException` | Temporary credentials expired | Refresh SSO session or rotate access keys |

!!! tip "Verify AWS credentials"

    ```bash
    aws sts get-caller-identity
    ```

    If this fails, your credentials are invalid or expired.

!!! tip "Check Bedrock model access"

    ```bash
    aws bedrock list-foundation-models --region us-east-1 \
      --query "modelSummaries[?modelId=='anthropic.claude-opus-4-6-v1'].modelId"
    ```

---

## General Debugging

### Enable verbose logging

Set `DEBUG=true` to see detailed request/response logs:

```bash
DEBUG=true airs runtime topics generate
```

### Structured output parse failures

All providers use `withStructuredOutput(ZodSchema)` with 3 automatic retries. If you see repeated parse failures:

- The model may not support structured output well -- try a larger model
- The schema constraints may be too tight -- check AIRS constraints (100 char name, 250 char description)
- `clampTopic()` handles post-LLM constraint enforcement, but extreme violations may still fail

