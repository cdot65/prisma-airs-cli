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

Prisma AIRS CLI is a CLI tool that provides full operational coverage over **Palo Alto Prisma AIRS** AI security capabilities — runtime prompt scanning and configuration management, custom topic guardrail commands, adversarial AI red teaming, ML model supply chain security, and multi-topic profile audits with conflict detection. Six LLM provider configurations are supported out of the box.

---

## Capabilities

<div class="grid cards" markdown>

-   :material-shield-search:{ .lg .middle } **Runtime Security**

    ---

    Scan prompts against live AIRS security profiles (sync and async), and manage runtime configuration — profiles, topics, API keys, customer apps, deployment/DLP profiles, and scan logs.

    [:octicons-arrow-right-24: Runtime Security](runtime/overview.md)

-   :material-refresh-auto:{ .lg .middle } **Guardrail Generation**

    ---

    Atomic `runtime topics` commands — create, apply, evaluate, and revert custom topic guardrails. An external agent orchestrates the create–apply–eval–revert loop to refine coverage against a static prompt set.

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

-   :material-magnify-scan:{ .lg .middle } **DLP Detection Testing**

    ---

    A synthetic, multi-modality corpus (PDF, JPEG, PNG, DOCX, ZIP) for evaluating how well a scanner detects sensitive data hidden via invisible text layers, metadata, container padding, OCR-only pixels, and steganography.

    [:octicons-arrow-right-24: DLP Detection Testing](dlp-detection/index.md)

</div>

---

## Platform Features

<div class="grid cards" markdown>

-   :material-brain:{ .lg .middle } **Multi-Provider LLM**

    ---

    Six provider configs — Claude API, Claude Vertex, Claude Bedrock, Gemini API, Gemini Vertex, Gemini Bedrock.

    [:octicons-arrow-right-24: LLM Providers](providers/overview.md)

-   :material-clipboard-text-search:{ .lg .middle } **DLP Detection**

    ---

    Generate, scan, and manage DLP filtering profiles, patterns, and dictionaries for sensitive-data detection.

    [:octicons-arrow-right-24: DLP](runtime/dlp/overview.md)

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

    System overview, core loop, and design decisions.

    [:octicons-arrow-right-24: Architecture](architecture/overview.md)

</div>
