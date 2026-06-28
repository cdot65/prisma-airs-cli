---
title: Prisma AIRS CLI
slug: /
hide_title: true
---

<div align="center">

<img src="/img/logo-wordmark-dark.svg" alt="prisma-airs-cli" width="440" style="max-width:100%;height:auto" />

**CLI and library for Palo Alto Prisma AIRS AI security**

[![CI](https://github.com/cdot65/prisma-airs-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/cdot65/prisma-airs-cli/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Node 20+](https://img.shields.io/badge/node-%3E%3D20-brightgreen.svg)](https://nodejs.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.x-3178c6.svg)](https://www.typescriptlang.org/)

</div>

---

Prisma AIRS CLI is a CLI tool that provides full operational coverage over **Palo Alto Prisma AIRS** AI security capabilities — runtime prompt scanning and configuration management, custom topic guardrail commands, adversarial AI red teaming, ML model supply chain security, and multi-topic profile audits with conflict detection. Six LLM provider configurations are supported out of the box.

## Capabilities

- **[Runtime Security](runtime/overview.md)** — Scan prompts against live AIRS security profiles (sync and async), and manage runtime configuration: profiles, topics, API keys, customer apps, deployment/DLP profiles, and scan logs.
- **[Guardrail Generation](runtime/guardrails/overview.md)** — Atomic `runtime topics` commands (create, apply, evaluate, revert). An external agent orchestrates the create–apply–eval–revert loop to refine coverage against a static prompt set.
- **[AI Red Teaming](redteam/overview.md)** — Launch static, dynamic, and custom adversarial scans against AI targets. Full CRUD for targets, prompt sets, and individual prompts with attack category filtering.
- **[Model Security](model-security/overview.md)** — ML model supply chain security: manage security groups, browse rules, configure rule instances, create scans, and review evaluations, violations, and file results.
- **[Profile Audits](runtime/profile-audits.md)** — Evaluate all topics in a security profile at once. Per-topic and composite metrics, cross-topic conflict detection, with terminal, JSON, and HTML output formats.
- **[DLP Detection Testing](dlp-detection/index.md)** — A synthetic, multi-modality corpus (PDF, JPEG, PNG, DOCX, ZIP) for evaluating how well a scanner detects sensitive data hidden via invisible text layers, metadata, container padding, OCR-only pixels, and steganography.

## Platform Features

- **[Multi-Provider LLM](providers/overview.md)** — Six provider configs: Claude API, Claude Vertex, Claude Bedrock, Gemini API, Gemini Vertex, Gemini Bedrock.
- **[DLP Detection](runtime/dlp/overview.md)** — Generate, scan, and manage DLP filtering profiles, patterns, and dictionaries for sensitive-data detection.

## Get Started

- **[Installation](getting-started/installation.mdx)** — Prerequisites, installation, and credential setup.
- **[Quick Start](getting-started/quick-start.md)** — Run your first command in minutes.
- **[Configuration](getting-started/configuration.md)** — LLM providers, tuning parameters, and data locations.
- **[Architecture](architecture/overview.md)** — System overview, core loop, and design decisions.
