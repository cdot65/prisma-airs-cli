# Repository authority and delivery

The canonical repository is https://git.cdot.io/cdot/prisma-airs-cli.
Open new issues and pull requests there. GitHub is a public source mirror;
historical GitHub pull requests remain available, and their open heads were
preserved as `archive/github-pr-N` branches in Forgejo.

Forgejo owns CI and release authorization. GitHub package and container workflows
are retained as `.disabled` files for historical reference; do not re-enable them.
GitHub Pages remains at its existing URL. Documentation is deployed only from an
`airs-docs-<full commit SHA>` tag approved in Forgejo and copied by the push mirror.
The GitHub Pages job verifies that the tag matches the exact source commit.

Package destinations remain npmjs.org for the SDK/CLI and npm.cdot.io for Harness.
Migration acceptance uses a separate prerelease channel and preserves `latest`.
The npm publishing credential is held at Conjur
`data/airs-release-bot/npmjs/token`; never put its value in source or logs.
Direct granular-token publishing is a temporary bridge: npm removes it in
January 2027. Before expiry, adopt staged releases with maintainer approval or a
supported trusted-publishing runner. See https://docs.npmjs.com/about-access-tokens/.

Clone the canonical source:

```sh
git clone git@git-ssh.cdot.io:cdot/prisma-airs-cli.git
```

Do not force-update a mirror until all destination-only commits and refs are
preserved. Stop mirroring before any emergency GitHub-authority rollback, then
reconcile both histories before resuming. Existing package versions are immutable.
