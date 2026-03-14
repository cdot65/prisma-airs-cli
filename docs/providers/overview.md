# LLM Providers

Prisma AIRS CLI supports **6 provider configurations** across three platforms. All use `temperature: 0` and LangChain's structured output with automatic retry on parse failure.

## Supported Providers

| Provider | SDK | Auth | Default Model |
|----------|-----|------|---------------|
| `claude-api` | `@langchain/anthropic` | API key | `claude-opus-4-6` |
| `claude-vertex` | `@anthropic-ai/vertex-sdk` + `@langchain/anthropic` | GCP ADC | `claude-opus-4-6` |
| `claude-bedrock` | `@langchain/aws` | IAM / default chain | `anthropic.claude-opus-4-6-v1` |
| `gemini-api` | `@langchain/google-genai` | API key | `gemini-2.5-pro` |
| `gemini-vertex` | `@langchain/google-vertexai` | GCP ADC | `gemini-2.5-pro` |
| `gemini-bedrock` | `@langchain/aws` | IAM / default chain | `gemini-2.5-pro` |

## Override the Default Model

Use `--model` or the `LLM_MODEL` env var:

```bash
# CLI flag
airs generate --model claude-sonnet-4-20250514

# Environment variable
export LLM_MODEL=claude-sonnet-4-20250514
```

!!! warning "Model naming differs per platform"
    Always use the correct format for your provider:

    | Platform | Example |
    |----------|---------|
    | Anthropic API | `claude-opus-4-6` |
    | Vertex AI (Claude) | `claude-opus-4-6` |
    | Bedrock (Claude) | `anthropic.claude-opus-4-6-v1` |
    | Google AI / Vertex / Bedrock (Gemini) | `gemini-2.5-pro` |

## Save Your Preference

Set your preferred provider in `~/.prisma-airs/config.json` to avoid repeating flags:

```json
{
  "llmProvider": "claude-api",
  "llmModel": "claude-opus-4-6"
}
```

---

## Which Provider Should I Use?

| Consideration | SaaS APIs | Vertex AI | Bedrock |
|---------------|-----------|-----------|---------|
| Setup difficulty | Easiest | Medium | Medium |
| Auth mechanism | API key | ADC (gcloud) | IAM / credential chain |
| Data residency | No control | GCP region | AWS region |
| Billing | Direct API | GCP project | AWS account |

For detailed setup, see:

- [SaaS APIs](saas-apis.md) — `claude-api`, `gemini-api`
- [Vertex AI](vertex-ai.md) — `claude-vertex`, `gemini-vertex`
- [AWS Bedrock](bedrock.md) — `claude-bedrock`, `gemini-bedrock`
