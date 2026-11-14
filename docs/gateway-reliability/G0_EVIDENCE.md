# Gateway reliability evidence ledger

Historical entry G0-source-1. For subsequent source implementation, see
[current progress](IMPLEMENTATION_PROGRESS.md). Implementation began with synthetic characterization
tests. Compilation, native test execution and environment admission are pending.
This ledger records source/data observations separately from test outcomes.

Plan: [Gateway reliability implementation plan](../GATEWAY_RELIABILITY_IMPLEMENTATION_PLAN.md).
Test module: [reliability.rs](../../tests/gateway_writes/reliability.rs), included
by [gateway_writes.rs](../../tests/gateway_writes.rs). It reuses the existing
fixture server, sandbox, executable selection and leak checks.

## Chunk status

| Chunk | Source work | Acceptance state |
| --- | --- | --- |
| G0.1 | Bound the CLI base revision, SDK archive/schema and existing installed artifacts below. Candidate tests select `CARGO_BIN_EXE_airs` explicitly. | Pending: admitted execution environment and newly built candidate executable/hash. Neither installed binary is selected for reproduction. |
| G0.2 | Added nine characterization tests covering the recorded attempts, independent blockers, valid local shapes, input-source behavior, credential-file handling and empty debug logs. | Pending: compile, formatting/lint and native execution. No green or red runtime result has been observed. |
| G0.3 | Retained explicit open questions below. Existing source establishes the local contract only. | Pending: organisation response shape, deployment write/omission semantics and service-key create vocabulary. |
| G1–G5 | Not started. | Pending their plan prerequisites. |

The preparation host is governed by [AGENTS.md](../../AGENTS.md) and
[SOURCE_PREPARATION.md](../SOURCE_PREPARATION.md):
“During source preparation do not run Cargo/native/app/live/provider” and
“No project build, application, live endpoint, real registration, native suite or
release command runs on Kirk during preparation.” No admitted execution location
has been supplied for these tests. A synthetic harness by itself does not admit
the build host. Native checks have therefore not been attempted here.

## Source and artifact bindings

CLI base revision: `e235a4d0aa431eb0929c129f0ae0f6d008b07d7c`.
The test addition and implementation-plan changes are working-tree changes on
that base. No implementation commit or built candidate identity is claimed.

| Baseline input | SHA-256 |
| --- | --- |
| `Cargo.toml` | `cf43541ae8812ef3310520703820066c2cdade8e199b47f0d599fee081a57652` |
| `Cargo.lock` | `969dc0d29bbac06e2c145a9a50b35adad5756ce5030344f8be7f2c235535455a` |
| `src/gateway/body.rs` | `2272c3b3f8ffe933b2b443b60aae00e9544122ace5f3bfbe91306afa30ee221c` |
| `src/commands/gateway_write_plan.rs` | `1015b0122dbff5840c9765bcec7170043901a1fdff63f4a563d5933413abab44` |
| `src/commands/gateway_writes.rs` | `ed8024510e6c0aa3784a585eb8116acde9f4fdcacae035f85d8721b6f741a6bf` |
| Locked `scm-sdk-0.2.4.crate` archive | `533294fe977e26e251e01dd2de64a896aa22ce45e97ac6264a0df6af879edd55` |
| Its `src/validation/schema_graph.json` | `f2a8fe0f1f3baa269b07d9732afa704f10358082947d7a5fd6ead44538e2aba1` |

The cached crate archive's SHA-256 matches `Cargo.lock`. Read-only inspection of
its embedded graph confirms strict deployment-auth input, a noncoercing optional
boolean for `allow_all_workspaces`, and required service-key `name`, `scopes`,
`organisation_id`, `workspace_id` and `type`. These are source/data observations,
not results from running the validator.

The sibling SDK working checkout was clean at
`c81b263c3214d4f5bd26099ffd6e318c2bcda346` during this entry. That checkout's
version is not the CLI's locked dependency; the tests use the locked crate.

Existing executable artifacts were read and hashed without execution:

| Artifact | Bytes | SHA-256 |
| --- | --- | --- |
| `/opt/homebrew/Cellar/airs-cli/9.0.0/bin/airs` | 36,983,824 | `e7c5f13e6fde97df6974df1c9e3b4cf2f13929912a033bcab7596d4823d1065e` |
| npm `@cdot65/prisma-airs-cli` bundled `vendor/aarch64-apple-darwin/airs` | 36,754,256 | `04206001e5709e751990671dc1e272f5dc914a3255eb6cccf95cce87a14b80b1` |

The npm artifact is below
`~/.nvm/versions/node/v24.21.0/lib/node_modules/@cdot65/prisma-airs-cli/node_modules/@cdot65/prisma-airs-cli-darwin-arm64/`.
These hashes identify existing installs only. They are not bindings to the new
test source or proof of behavioral differences between installations.

## Reproductions prepared for the admitted environment

The nine tests below are in the `reliability::` module of the `gateway_writes`
integration-test target. Every runtime result is **not run**.

| Test | Expected baseline signal |
| --- | --- |
| `recorded_deployment_attempts_fail_before_loading_a_tenant` | All three synthetic translations of the incident attempts exit 2 with the schema-error route and zero OAuth/API requests, even without a tenant. |
| `deployment_blockers_are_independent_and_grouped_url_is_not_a_fix` | Numeric boolean alone, the read-only field with a corrected boolean, and the suggested grouped URL each independently fail local validation. |
| `read_auth_settings_need_both_type_and_field_changes_for_the_write_schema` | The existing read fixture fails the real SDK parser; changing the boolean alone still fails; removing the read-only field produces a valid local write shape. |
| `a_full_deployment_get_document_is_not_a_write_request_file` | Supplying the full synthetic GET document through `--file` fails before authentication and does not expose fixture credentials. |
| `write_shaped_deployment_settings_reach_the_fixture_once` | A corrected explicit write body reaches the fixture unchanged in one PUT after one token request. This does not establish service merge semantics. |
| `missing_key_type_is_rejected_without_a_tenant_or_secret_file` | The recorded command shape fails before tenant loading; the valid seconds-only expiry does not hide the missing-type refusal; no credential file or temporary exists. |
| `key_type_can_come_from_any_input_source_with_either_timestamp_form` | Flag, file and assignment sources pass local preparation with seconds-only and fractional timestamps. The recorded POSTs omit alert emails and the canned one-time responses are saved privately. |
| `service_key_type_obeys_file_flag_assignment_precedence` | File < named flag < assignment produces the final assigned type and one POST. |
| `semantic_schema_rejections_currently_create_empty_debug_logs` | Deployment and key semantic rejections create a zero-byte log and report the schema error, with zero network requests and no credential output. G3.2 must replace this characterization with the intended validation-event assertions. |

Inputs use fixture UUIDs, TSG `100`, `gw.example.test`, synthetic workspace names
and the existing synthetic credential markers. None is a production recording.
The positive key tests deliberately use `workspace-service` only to characterize
the pinned SDK's nonempty-string acceptance. The canned fixture response is not
evidence that a live service accepts that create value.

In an admitted environment with its approved dependencies and toolchains, run:

```sh
cargo fmt --all --check
cargo test --locked --offline -p airs-cli --test gateway_writes reliability:: -- --test-threads=1
cargo clippy --locked --offline -p airs-cli --test gateway_writes -- -D warnings
```

These commands are a pending execution recipe, not executed evidence. Bind their
working source diff and actual environment when running them. The integration
target spawns `env!("CARGO_BIN_EXE_airs")`, never an ambient PATH or installed
release binary. After the build, record that executable's absolute path, SHA-256,
compiler/target and the exact result of each test. A missing offline input remains
an input failure; do not silently switch dependencies or fetch an unreviewed one.

## Preparation checks performed

`git diff --check` passed for the tracked change. Read-only Python source/data
checks also passed: all nine tests are included and named in this ledger, local
Markdown links resolve, all changed/added text has clean whitespace, and the
baseline hashes above match the inspected inputs. The SDK graph was read from
the checksum-bound archive rather than reimplemented or executed. These checks
do not establish Rust syntax, formatting, compilation or test outcomes.

## Desired diagnostic assertions for the next phases

The baseline tests characterize the local contract and incident symptoms; they
do not claim that the desired diagnostics already exist. Before implementing G1
and G3 in the admitted environment, make the new diagnostic assertions fail on
the baseline, then pass with the fix:

- Numeric workspace permission: a schema-owned path to
  `auth_settings.allow_all_workspaces`, expected boolean and received number.
- Missing key type: `type`, required/absent, with a CLI hint supporting flag,
  file and assignment input rather than a Clap-only requirement.
- Unsupported authentication field: safe identification of the containing
  `auth_settings` object and allowed fields; arbitrary input key names withheld.
- Local schema rejection with debug enabled: a metadata validation event with
  `requestSent: false`, while the existing zero-traffic/file assertions remain.
- Response failures: separate JSON, schema and model stages while retaining
  post-send outcome and one-time-credential preservation.

## Open service evidence

**Organisation info:** the incident handoff contains the generic error, not a
response. Obtain a redacted field/type/nullability shape or an admitted diagnostic
capture before naming the offending constraint. Do not invent a purported
incident fixture by deleting a field from the current synthetic response.

**Deployment updates:** the returned record establishes stored representation,
not which fields the console sent or whether omitted authentication settings are
merged or replaced. The new positive test proves a local write shape only.

**Service keys:** confirm the supported create vocabulary independently of the
console-created key's returned type and the CLI's canned fixture success.

No production endpoint or real tenant/configuration/credential store was read
while preparing this entry. No new service observation, SDK change, application
behavior change or phase acceptance is claimed.
