# Vertex AI

Run models through Google Cloud's Vertex AI platform. Both `claude-vertex` and `gemini-vertex` use **Application Default Credentials (ADC)** — no API keys to manage.

## Prerequisites

1. A Google Cloud project with the Vertex AI API enabled
2. Application Default Credentials configured:

```bash
gcloud auth application-default login
```

!!! info "IAM Roles"

    Your account needs the **Vertex AI User** role (`roles/aiplatform.user`) on the GCP project.

## Environment Variables

| Env Var | Required | Default | Description |
|---------|----------|---------|-------------|
| `GOOGLE_CLOUD_PROJECT` | Yes | -- | GCP project ID |
| `GOOGLE_CLOUD_LOCATION` | No | `us-central1` (gemini) / `global` (claude) | GCP region |

---

## claude-vertex

Claude models on Vertex AI via `@anthropic-ai/vertex-sdk` with the `createClient` pattern, wrapped by `@langchain/anthropic`.

!!! warning "Default region is `global`"

    Unlike other Vertex AI models, Claude on Vertex defaults to the `global` region, **not** `us-central1`. Set `GOOGLE_CLOUD_LOCATION` explicitly if you need a specific region.

### Model Naming

Model names match the Anthropic API format -- no special Vertex-specific naming required.

| API Name | Vertex Name |
|----------|-------------|
| `claude-opus-4-6` | `claude-opus-4-6` |
| `claude-sonnet-4-20250514` | `claude-sonnet-4-20250514` |

### Setup

=== "Environment File"

    ```bash title=".env"
    LLM_PROVIDER=claude-vertex
    GOOGLE_CLOUD_PROJECT=my-gcp-project-id
    # GOOGLE_CLOUD_LOCATION=global  # default for claude-vertex
    ```

=== "Export"

    ```bash
    export LLM_PROVIDER=claude-vertex
    export GOOGLE_CLOUD_PROJECT=my-gcp-project-id
    ```

### Usage

```bash
pnpm run generate -- --provider claude-vertex

# Override region
GOOGLE_CLOUD_LOCATION=us-east5 pnpm run generate -- --provider claude-vertex
```

---

## gemini-vertex

Gemini models on Vertex AI via `@langchain/google-vertexai` (`ChatVertexAI`). The GCP project is passed via `authOptions.projectId`.

### Setup

=== "Environment File"

    ```bash title=".env"
    LLM_PROVIDER=gemini-vertex
    GOOGLE_CLOUD_PROJECT=my-gcp-project-id
    # GOOGLE_CLOUD_LOCATION=us-central1  # default for gemini-vertex
    ```

=== "Export"

    ```bash
    export LLM_PROVIDER=gemini-vertex
    export GOOGLE_CLOUD_PROJECT=my-gcp-project-id
    ```

### Usage

```bash
pnpm run generate -- --provider gemini-vertex

# Override model and region
pnpm run generate -- --provider gemini-vertex --model gemini-2.5-pro \
  GOOGLE_CLOUD_LOCATION=europe-west1
```

---

## Region Availability

!!! tip "Check model availability by region"

    Not all models are available in all regions. Consult the Vertex AI documentation for current regional availability:

    - [Claude on Vertex AI regions](https://cloud.google.com/vertex-ai/docs/partner-models/use-claude#regions)
    - [Gemini on Vertex AI regions](https://cloud.google.com/vertex-ai/docs/generative-ai/learn/locations)
