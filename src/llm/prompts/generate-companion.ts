import { ChatPromptTemplate } from '@langchain/core/prompts';

export const generateCompanionPrompt = ChatPromptTemplate.fromMessages([
  [
    'system',
    `You are an expert at configuring Prisma AIRS custom topic guardrails. Your task is to create a DOMAIN-SPECIFIC allow topic that covers the BENIGN, LEGITIMATE side of the same vocabulary domain as the block topic.

CRITICAL ARCHITECTURE REQUIREMENT:
AIRS topic-guardrails need BOTH allow and block topics that SHARE THE SAME VOCABULARY DOMAIN but differ in intent. The allow topic establishes the benign boundary; the block topic carves out malicious content. Without a domain-overlapping allow topic, AIRS cannot distinguish benign from malicious prompts.

EXAMPLE — Home improvement chatbot blocking violence:
- Allow: "Household Tools" — household tools (automotive, cleaning, electrical, hand tools, etc.)
- Block: "Usage of Household Tools for Violence" — using household items for violent actions
Both share vocabulary (hammers, paint, rope, chemicals) but differ in intent.

EXAMPLE — Blocking tax evasion:
- Allow: "Legal Tax Planning" — tax education, compliance, retirement accounts (IRA, 401k), filing, deduction eligibility, legitimate tax optimization
- Block: "Tax Evasion" — hiding income, fraudulent deductions, offshore evasion schemes
Both share vocabulary (taxes, deductions, income, accounts) but differ in intent.

The allow topic MUST:
- Share vocabulary/domain with the block topic (this is what gives AIRS the semantic signal)
- Describe the BENIGN, LEGITIMATE intent within that domain
- Cover education, compliance, legal use cases for the same subject matter
- Be specific enough for AIRS semantic matching (NOT generic "everyday content")

Constraints:
- Name: max 100 characters, prefix with "Allow: "
- Description: max 250 characters — USE THE FULL BUDGET for precision
- Examples: 2 examples of clearly benign prompts that use the SAME DOMAIN VOCABULARY as the block topic
- Combined total (name + description + all examples) must not exceed 1000 characters`,
  ],
  [
    'human',
    `Create a domain-specific allow companion topic for:

Block Topic: {blockTopicName}
Block Description: {blockTopicDescription}

The allow topic must share vocabulary with the block topic but cover the BENIGN, LEGITIMATE side of that domain. Think: education, compliance, legal use, professional guidance within the same subject area.`,
  ],
]);
