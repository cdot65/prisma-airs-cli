import { ChatPromptTemplate } from '@langchain/core/prompts';

export const generateTopicPrompt = ChatPromptTemplate.fromMessages([
  [
    'system',
    `You are an expert at configuring Prisma AIRS custom topic guardrails. Your job is to create a topic definition that will effectively {intent} conversations about the described topic.

Constraints:
- Name: max 100 characters, concise and descriptive
- Description: max 250 characters, clearly defines what should be detected
- Examples: 2-5 examples, each max 250 characters
- Combined total (name + description + all examples) must not exceed 1000 characters

Example count strategy:
- The description field carries the most weight in AIRS topic matching. A precise, well-crafted description can outperform a vague description with many examples.
- Use 2 examples when a couple of representative phrases clarify the boundary and the description is strong.
- Use 3-5 examples when the topic has diverse surface forms that the description alone cannot capture.
- More examples are NOT always better — they can broaden matching unpredictably and increase false positives.
- Vary example count between iterations to discover the optimal configuration.

Intent: {intent}
- "block" (blacklist): Cast a wide net. The description should broadly capture the prohibited topic. Examples should cover diverse phrasings, indirect references, and coded language.
- "allow" (whitelist): Be precise and narrow. The description must tightly define what is permitted. Examples should represent the exact range of allowed content without overreaching. Fewer, sharper examples often outperform many broad ones.
{memorySection}`,
  ],
  [
    'human',
    `Create a custom topic guardrail to {intent} the following:

Topic: {topicDescription}

{seedExamplesSection}

Generate a topic definition with a name, description, and between 2-5 examples. Choose the example count that maximizes efficacy for a {intent} guardrail.`,
  ],
]);

export function buildSeedExamplesSection(seeds?: string[]): string {
  if (!seeds || seeds.length === 0) return '';
  return `Seed examples to build upon:\n${seeds.map((s, i) => `${i + 1}. ${s}`).join('\n')}`;
}
