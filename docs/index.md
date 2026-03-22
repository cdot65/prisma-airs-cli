---
title: Home
---

<div class="hero" markdown>

![Prisma AIRS CLI](images/logo.svg){ .hero-logo }

# Prisma AIRS CLI

**CLI and library for Palo Alto Prisma AIRS AI security**

[![CI](https://github.com/cdot65/prisma-airs-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/cdot65/prisma-airs-cli/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Node 20+](https://img.shields.io/badge/node-%3E%3D20-brightgreen.svg)](https://nodejs.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.x-3178c6.svg)](https://www.typescriptlang.org/)

</div>

---

Prisma AIRS CLI is a CLI tool that provides full operational coverage over **Palo Alto Prisma AIRS** AI security capabilities — runtime prompt scanning and configuration management, LLM-driven guardrail generation, adversarial AI red teaming, ML model supply chain security, and multi-topic profile audits with conflict detection. Six LLM provider configurations are supported out of the box.

---

## Capabilities

<div class="grid cards" markdown>

-   :material-shield-search:{ .lg .middle } **Runtime Security**

    ---

    Scan prompts against live AIRS security profiles (sync and async), and manage runtime configuration — profiles, topics, API keys, customer apps, deployment/DLP profiles, and scan logs.

    [:octicons-arrow-right-24: Runtime Security](runtime/overview.md)

-   :material-refresh-auto:{ .lg .middle } **Guardrail Generation**

    ---

    LLM-driven feedback loop that generates, deploys, tests, evaluates, and iteratively refines custom topic guardrails until coverage meets your target threshold.

    [:octicons-arrow-right-24: Guardrail Generation](runtime/guardrails/overview.md)

-   :material-sword:{ .lg .middle } **AI Red Teaming**

    ---

    Launch static, dynamic, and custom adversarial scans against AI targets. Full CRUD for targets, prompt sets, and individual prompts with attack category filtering.

    [:octicons-arrow-right-24: AI Red Teaming](redteam/overview.md)

-   :material-shield-lock:{ .lg .middle } **Model Security**

    ---

    ML model supply chain security — manage security groups, browse rules, configure rule instances, create scans, and review evaluations, violations, and file results.

    [:octicons-arrow-right-24: Model Security](model-security/overview.md)

-   :material-clipboard-check:{ .lg .middle } **Profile Audits**

    ---

    Evaluate all topics in a security profile at once. Per-topic and composite metrics, cross-topic conflict detection, with terminal, JSON, and HTML output formats.

    [:octicons-arrow-right-24: Profile Audits](runtime/profile-audits.md)

</div>

---

## Platform Features

<div class="grid cards" markdown>

-   :material-brain:{ .lg .middle } **Multi-Provider LLM**

    ---

    Six provider configs — Claude API, Claude Vertex, Claude Bedrock, Gemini API, Gemini Vertex, Gemini Bedrock.

    [:octicons-arrow-right-24: LLM Providers](providers/overview.md)

-   :material-memory:{ .lg .middle } **Cross-Run Memory**

    ---

    Persists learnings across guardrail runs so the LLM avoids repeating past mistakes. Budget-aware injection keeps prompts focused.

-   :material-play-pause:{ .lg .middle } **Resumable Runs**

    ---

    Every guardrail iteration checkpoints to disk. Resume failed or paused runs from exactly where they left off.

-   :material-test-tube:{ .lg .middle } **Test Composition**

    ---

    Carried failures, regression tiers, and fresh LLM-generated tests are composed each iteration with weighted category generation.

</div>

---

## Get Started

<div class="grid cards" markdown>

-   :material-download:{ .lg .middle } **Install**

    ---

    Prerequisites, installation, and credential setup.

    [:octicons-arrow-right-24: Installation](getting-started/installation.md)

-   :material-rocket-launch:{ .lg .middle } **Quick Start**

    ---

    Run your first command in minutes.

    [:octicons-arrow-right-24: Quick Start](getting-started/quick-start.md)

-   :material-cog:{ .lg .middle } **Configure**

    ---

    LLM providers, tuning parameters, and data locations.

    [:octicons-arrow-right-24: Configuration](getting-started/configuration.md)

-   :material-book-open-variant:{ .lg .middle } **Architecture**

    ---

    System overview, core loop, memory system, and design decisions.

    [:octicons-arrow-right-24: Architecture](architecture/overview.md)

</div>
