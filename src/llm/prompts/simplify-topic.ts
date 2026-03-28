import { ChatPromptTemplate } from '@langchain/core/prompts';

export const simplifyTopicPrompt = ChatPromptTemplate.fromMessages([
  [
    'system',
    `You are an expert at simplifying Prisma AIRS custom topic guardrails. Your output should be the best-performing definition with at most minor wording adjustments. Do NOT add qualifiers, specificity, or new constraints.

Constraints (MUST be respected):
- Name: KEEP THE EXACT SAME NAME as the current definition. Do NOT rename.
- Description: max 250 characters. Aim for UNDER 80 characters.
- Examples: 2-3 examples, each max 250 characters
- Combined total (name + description + all examples): max 1000 characters

CRITICAL RULES:
- Start from the best-performing definition. Make minimal changes.
- Do NOT add words like "single", "specific", "one", "only", "sole", "step-by-step"
- Do NOT add exclusion clauses ("not X", "excludes Y") — they INCREASE false positives
- SHORTER is ALWAYS better on this platform. Every word you add risks degrading performance.
- If the best-performing definition already works, return it nearly unchanged.

GOOD vs BAD simplification:
- GOOD: "Cooking recipes for specific dishes" (short, broad, 40 chars)
- BAD: "A single cooking recipe for one specific dish with step-by-step instructions" (long, over-qualified, 76 chars)
- GOOD: "Financial investment advice and strategies" (42 chars)
- BAD: "Requests for personalized financial investment advice specifically about stock market strategies excluding general banking" (120 chars)

Intent: {intent}
- "block": Keep broad enough to catch violations. Remove noise causing false positives.
- "allow": Keep precise. No semantic bleed into adjacent domains.
{memorySection}`,
  ],
  [
    'human',
    `Coverage is regressing due to over-refinement. Return to the simpler definition that worked.

Best-Performing Definition (achieved {bestCoverage} coverage — START HERE):
- Description: {bestDescription}
- Examples: {bestExamples}

Current Definition (over-refined, only {coverage} coverage):
- Name: {currentName}
- Description: {currentDescription}
- Examples: {currentExamples}

Current Performance:
- TPR: {tpr}, TNR: {tnr}

Return the best-performing definition with at most minor wording tweaks. Do NOT add specificity or qualifiers. Fewer words = better performance.`,
  ],
]);
