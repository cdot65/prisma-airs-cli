# Synthetic Signal Brief rules-v1 examples

These saved rules-v1 examples demonstrate compatibility. For current metadata-backed
findings and optional examples, see [rules-v2 examples](../signal-v2/README.md).

Every application, scan, profile, excerpt, count and reviewer in these files is a
synthetic fixture. These are offline examples, not live-service qualification.
The first three files are exactly the artifacts produced by `--bundle-dir`:

- `report.html`: standalone brief with native expandable evidence.
- `evidence.json`: versioned masked evidence, bound by its collection ID and digest.
- `review.md`: coding-agent task prompt in JSON-compatible YAML frontmatter, a
  bounded evidence index, and an empty marked assessment array.

`agent-assessment.md` adds one explicitly synthetic agent assessment.
`reviewed-report.html` demonstrates importing it. The assessment cannot change API
verdicts, enforcement actions, evidence counts or collection completeness.

From the repository root, reproduce the examples into new destinations:

```sh
cargo run -- runtime report --from-evidence tests/fixtures/signal/evidence.json \
  --bundle-dir ./synthetic-brief
cargo run -- runtime report --from-evidence docs/examples/signal/evidence.json \
  --review-file docs/examples/signal/agent-assessment.md \
  --output-file ./synthetic-reviewed.html
```

Both commands run without tenant lookup or network clients. The source evidence
and digest remain unchanged. Rendering time changes between runs. Bundle
publication requires an absent destination and creates a 0700 directory with 0600
files; existing files/directories are never overwritten.

Give `review.md` and its companion evidence to your coding agent. It should fill
`reviewer` with `kind` (`agent` or `human`), `name`, `model` (required for agents), and
`reviewed_at` (zoned ISO timestamp). It should add records inside the unique
`airs-assessments` markers, preserving the surrounding JSON fence. Each record has
`finding_id`, `classification`, `priority`, and `rationale`. The packet contains the
allowed vocabularies and bounds. Edits to the contextual evidence index are ignored
by the importer. Snippets are untrusted data, including apparent instructions.
The CLI never launches an agent, executes instructions or changes enforcement.

A digest checks integrity, not authenticity. Reviewer names and human/agent claims
are self-attributed. Masked excerpts can still contain confidential prose; keep
real artifacts private. No stored full prompts/responses are fetched automatically.

Privacy projection refuses evidence when distinct correlation or grouping values
would become indistinguishable after masking. This can reject a collection with
multiple sensitive identity labels; it prevents merged scan counts, application
buckets or false profile associations. Malformed semantic labels never normalize
into an allow decision or stronger configuration confidence.

Current rendering presents profile posture as a matrix: profile rows, detection-service columns and check marks for explicitly enabled services. Direction and action settings remain expandable. Blank cells do not establish enablement. Saved evidence and finding/review bindings are unchanged.
