---
title: Library — Recipes
---

# Library — Recipes

Short, complete code samples for common library tasks. Each recipe is self-contained and uses
the real constructor/method signatures from the source.

---

## Recipe 1: List security profiles

Uses [`SdkManagementService`](../api/classes/SdkManagementService.md).

`SdkManagementService` constructor signature:

```typescript
constructor(opts?: ManagementClientOptions)
// ManagementClientOptions fields (all optional, fall back to env vars):
//   clientId?:      string  — PANW_MGMT_CLIENT_ID
//   clientSecret?:  string  — PANW_MGMT_CLIENT_SECRET
//   tsgId?:         string  — PANW_MGMT_TSG_ID
//   tokenEndpoint?: string  — PANW_MGMT_TOKEN_ENDPOINT
//   apiEndpoint?:   string  — PANW_MGMT_ENDPOINT
//   numRetries?:    number  — default 5
```

```typescript
import { SdkManagementService } from '@cdot65/prisma-airs-cli';

// Credentials are picked up from PANW_MGMT_* env vars automatically.
const mgmt = new SdkManagementService();

const { profiles } = await mgmt.listProfiles();

for (const p of profiles) {
  console.log(`${p.profileName}  (id: ${p.profileId})`);
}
```

To pass credentials explicitly instead of via env vars:

```typescript
const mgmt = new SdkManagementService({
  clientId: 'my-client-id',
  clientSecret: 'my-client-secret',
  tsgId: 'my-tsg-id',
});
```

---

## Recipe 2: Scan a prompt and handle the result

Uses [`SdkRuntimeService`](../api/classes/SdkRuntimeService.md).

`SdkRuntimeService` constructor signature:

```typescript
constructor(apiKey: string)
```

```typescript
import { SdkRuntimeService, type RuntimeScanResult } from '@cdot65/prisma-airs-cli';

const runtime = new SdkRuntimeService(process.env.PANW_AI_SEC_API_KEY!);

async function checkPrompt(profile: string, userInput: string): Promise<void> {
  const result: RuntimeScanResult = await runtime.scanPrompt(profile, userInput);

  if (result.action === 'block') {
    console.error('Prompt blocked. Detections:', result.detections);
    return;
  }

  console.log('Prompt allowed. Scan ID:', result.scanId);
}

await checkPrompt('prod-security-profile', 'Summarise the quarterly earnings report.');
```

To also scan the model **response** alongside the prompt, pass it as the optional third argument:

```typescript
const result = await runtime.scanPrompt(
  'prod-security-profile',
  userPrompt,
  modelResponse,
);
```

---

## Recipe 3: List custom topics on a profile

Uses [`SdkManagementService`](../api/classes/SdkManagementService.md).

```typescript
import { SdkManagementService } from '@cdot65/prisma-airs-cli';

const mgmt = new SdkManagementService();

// List all custom topics defined in the tenant.
const topics = await mgmt.listTopics();

console.log(`${topics.length} custom topic(s):`);
for (const t of topics) {
  console.log(`  ${t.topic_name}  rev=${t.revision}  id=${t.topic_id}`);
}

// Fetch a single topic by name.
const topic = await mgmt.getTopicByName('off-topic-finance');
console.log('Description:', topic.description);
```

---

## Further reading

- [API Reference index](../api/README.md) — TypeDoc for every exported class and type
- [SdkRuntimeService](../api/classes/SdkRuntimeService.md)
- [SdkManagementService](../api/classes/SdkManagementService.md)
