# AWS Bedrock

Run models through AWS Bedrock. Both `claude-bedrock` and `gemini-bedrock` use `@langchain/aws` and support either explicit credentials or the AWS default credential chain.

## Authentication

Bedrock supports two authentication methods:

=== "Explicit Credentials"

    ```bash title=".env"
    AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE
    AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
    AWS_REGION=us-east-1
    ```

=== "Default Credential Chain"

    If no explicit credentials are set, the AWS SDK uses the [default credential chain](https://docs.aws.amazon.com/sdk-for-javascript/v3/developer-guide/setting-credentials-node.html):

    1. Environment variables
    2. Shared credentials file (`~/.aws/credentials`)
    3. SSO credentials
    4. EC2/ECS instance role
    5. And more...

    ```bash
    # Only region needed if using default chain
    export AWS_REGION=us-east-1
    ```

## Environment Variables

| Env Var | Required | Default | Description |
|---------|----------|---------|-------------|
| `AWS_REGION` | No | `us-east-1` | AWS region |
| `AWS_ACCESS_KEY_ID` | No* | -- | IAM access key |
| `AWS_SECRET_ACCESS_KEY` | No* | -- | IAM secret key |

!!! info "* Credential requirement"

    `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` are not required if you are using the AWS default credential chain (e.g., IAM role, SSO, `~/.aws/credentials`).

---

## claude-bedrock

Claude models on AWS Bedrock.

### Model Naming

!!! warning "Bedrock uses full model IDs"

    Bedrock model IDs differ from the Anthropic API. Use the full ARN-style identifier.

| Anthropic API | Bedrock Model ID |
|---------------|------------------|
| `claude-opus-4-6` | `anthropic.claude-opus-4-6-v1` |
| `claude-sonnet-4-20250514` | `anthropic.claude-sonnet-4-20250514-v1` |

### Setup

=== "Environment File"

    ```bash title=".env"
    LLM_PROVIDER=claude-bedrock
    AWS_REGION=us-east-1
    AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE
    AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
    ```

=== "Default Credential Chain"

    ```bash title=".env"
    LLM_PROVIDER=claude-bedrock
    AWS_REGION=us-east-1
    ```

### Usage

```bash
airs runtime topics generate --provider claude-bedrock

# Override model
airs runtime topics generate --provider claude-bedrock --model anthropic.claude-sonnet-4-20250514-v1
```

---

## gemini-bedrock

Gemini models on AWS Bedrock, also via `@langchain/aws` (`ChatBedrockConverse`).

### Setup

=== "Environment File"

    ```bash title=".env"
    LLM_PROVIDER=gemini-bedrock
    AWS_REGION=us-east-1
    AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE
    AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
    ```

=== "Default Credential Chain"

    ```bash title=".env"
    LLM_PROVIDER=gemini-bedrock
    AWS_REGION=us-east-1
    ```

### Usage

```bash
airs runtime topics generate --provider gemini-bedrock

# Override model
airs runtime topics generate --provider gemini-bedrock --model gemini-2.5-pro
```

---

## Prerequisites

!!! tip "Enable models in the Bedrock console"

    Before using any model on Bedrock, you must **enable access** in the AWS Bedrock console:

    1. Open the [AWS Bedrock console](https://console.aws.amazon.com/bedrock/)
    2. Navigate to **Model access**
    3. Request access to the desired model(s)
    4. Wait for approval (Claude models may require a brief approval period)

!!! info "Region availability"

    Not all models are available in all AWS regions. `us-east-1` and `us-west-2` have the broadest model support.
