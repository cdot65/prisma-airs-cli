import { describe, expect, it } from 'vitest';
import { createLlmProvider } from '../../src/llm/provider.js';

const GOOGLE_CLOUD_PROJECT = process.env.GOOGLE_CLOUD_PROJECT;

describe.skipIf(!GOOGLE_CLOUD_PROJECT)('Vertex AI providers (e2e)', () => {
  it('claude-vertex: invoke simple prompt', async () => {
    const provider = await createLlmProvider({
      provider: 'claude-vertex',
      googleCloudProject: GOOGLE_CLOUD_PROJECT,
      googleCloudLocation: process.env.CLOUD_ML_REGION ?? 'global',
    });
    const response = await provider.invoke('Say "hello" and nothing else.');
    expect(response.content).toBeTruthy();
  }, 30_000);

  it('gemini-vertex: invoke simple prompt', async () => {
    const provider = await createLlmProvider({
      provider: 'gemini-vertex',
      googleCloudProject: GOOGLE_CLOUD_PROJECT,
      googleCloudLocation: process.env.GOOGLE_CLOUD_LOCATION ?? 'us-central1',
    });
    const response = await provider.invoke('Say "hello" and nothing else.');
    expect(response.content).toBeTruthy();
  }, 30_000);
});
