# Changesets

Add a changeset file here when making user-facing changes. The `/release` skill consumes these files to generate release notes.

## Format

Create a file named `0000-short-description.md`:

```md
---
"@cdot65/prisma-airs-cli": patch
---

Description of the change (user-facing).
```

Use `patch`, `minor`, or `major` to indicate the semver bump level. The highest level across all changesets determines the release bump.
