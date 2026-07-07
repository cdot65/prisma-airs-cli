---
"@cdot65/prisma-airs-cli": patch
---

Fixed the Docker image build: tsup.config.ts is now copied into the build stage (the v3.0.0 image build failed with "No input files"; the npm package was unaffected).
