---
title: Moving to airs-cli in version 7
---

Version 7 renames the product command from `airs` to **`airs-cli`**. Its npm
package remains `@cdot65/prisma-airs-cli`. The Prisma AIRS Harness takes the
`airs` command and runs its bundled product CLI through **`airs cli ...`**.

| Before | Standalone CLI 7 | Harness with CLI 7 bundled |
| --- | --- | --- |
| `airs runtime profiles list` | `airs-cli runtime profiles list` | `airs cli runtime profiles list` |
| `airs tenant switch development` | `airs-cli tenant switch development` | `airs cli tenant switch development` |
| `airs doctor` | `airs-cli doctor` | `airs cli doctor` |

`airs doctor` now means harness diagnostics. Update scripts, scheduled jobs,
shell aliases, service definitions and user-created skills before changing their
installed command. Product config files and tenant registrations stay in place.

As of September 20, 2026, standalone **7.0.1** is the stable release for default
public npm installs. **7.1.5** is published under `next` and adds the
[TypeSafe Jev red-team judge](../cli/redteam/judge.md), including explicit coverage
exclusions, validated provider responses and offline replay.

The harness bundles an independently pinned CLI. Installing a newer global
`airs-cli` does not upgrade `airs cli`. Check both with `airs-cli --version` and
`airs cli --version`. Use **7.1.5** for the shared TypeScript judge and official Jev SDK. Model output strings remain verbatim.
Harness **0.1.2-alpha.5.mcp.1** bundles CLI **7.1.5** and the matching native skill.
Install the exact preview with
`npm install -g airs-harness@0.1.2-alpha.5.mcp.1 --registry=https://npm.cdot.io`.
Installing standalone 7.1.5 does not update the CLI or skill embedded inside an
existing harness. Restart `airs` after upgrading to load the updated native skill.
Use `/typesafe` inside the harness to configure the optional judge key without an external command.
The harness's stable `latest` remains **0.1.1**, bundling CLI 7.0.0.

## Upgrade an existing standalone installation

Upgrade the CLI **before** installing the renamed harness so it releases `airs`:

```sh
npm install -g @cdot65/prisma-airs-cli@7.0.1
airs-cli --version
airs-cli tenant list
```

To try the judge preview instead, install the exact version:

```sh
npm install -g @cdot65/prisma-airs-cli@7.1.5
airs-cli redteam judge --help
```

Then install the harness version supplied by your organization, with npm optional
dependencies enabled. The harness installation bundles its own tested CLI; users
who only need the harness do not need a separate global `airs-cli` installation.

Open a fresh shell and check `type -a airs airs-cli airs-harness`. An old alias,
linked development checkout, another Node installation or a manual executable
can still own `airs` earlier on PATH. Resolve it through its original installer.
Do not use `npm --force` to overwrite an unknown executable.

Regenerate standalone shell completions with `airs-cli completion SHELL`.
Remove the old product completion registration for `airs` when installing the
harness's completion. Use `airs completion SHELL` for the harness, including its
nested product commands.

## Configuration and authentication

The command rename does not alter CLI 6 tenant configuration. Harness users
upgrading from the older bundled CLI 5.2 need to register a trusted config file:

```sh
airs cli tenant create development --config /absolute/path/to/config.json
airs cli tenant switch development
airs cli doctor --output json
```

Alternatively, `airs cli tenant create development` prompts for credentials with
hidden secret input. Credential environment variables and working-directory
`.env` files are not configuration sources. Never paste credentials into chat.

Harness environments and CLI tenants have independent selections. Company SSO
authorizes gateway inference and MCP access; it does not provide product API
service-account credentials to this local CLI.

## Rollback

The renamed standalone CLI can coexist with harness alpha.21, whose command is
`airs-harness`. To restore CLI 6's ownership of `airs`, first downgrade or remove
the new harness through its package manager, then reinstall CLI 6.1.1. Preserve
config files and tenant registrations. Changing registry tags does not downgrade
packages already installed on a workstation.
