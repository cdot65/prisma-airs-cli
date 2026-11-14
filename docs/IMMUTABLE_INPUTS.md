# Immutable CLI inputs and consumer reconciliation

This source contract records current requirements separately from historical
inputs. It does not publish an input, qualify a runtime or authorize a channel
change. Exact original observations remain in the
[initial import contract](https://git.cdot.io/goshawks.io/prisma-airs-cli/src/commit/bdc5a7f78706dc005e674dfde50922af595646b1/docs/IMMUTABLE_INPUTS.md).

| Input or consumer | Current treatment |
| --- | --- |
| Historical TypeScript7a54 reference/parity input | Retired for current CLI9.0: upstream removed `scripts/build-reference-cli.sh`, `Cli::ts`, the TypeScript/live parity machinery and its tests. Do not provision this obsolete current dependency. Preserve original source/input provenance for retained older versions or consumers until their recovery obligations end. |
| M7 older-binary refusal | Still retained: `scripts/build-pre-m7-cli.sh` and `AIRS_PRE_M7_BIN` in `tests/mcp_client.rs` use the exact older8.0 default `f3e61ac080b6c5dac154f6837f1610e824c49b89`. Admit a recoverable immutable source/toolchain/binary recipe before this affected execution or old-history retirement; the new root cannot supply those old Git objects. |
| Synthetic RSA/withholding inputs | Committed fixtures and corpus remain exact, not production credentials. Preserve their trust/refusal relationships; never substitute a real tenant store. |
| Stable source/formula rendering | Current source9.0.0 is stable, but source reconciliation does not authorize rendering/publishing a replacement formula or repackaging an existing version. Existing Cargo/npm/registry identities remain unchanged. |
| Existing8.2.0 and9.0.0 release assets | Already published upstream. Preserve their identities and bytes. No duplicate/conflicting publication or new release is implied. |
| Imported Homebrew source and existing public tap | Independent source/history remains separate. No formula, version, URL, hash or channel is changed by this reconciliation. Verify live publisher state at its deliberate cutover. |
| Frozen DesktopCLI8.0 input | Keep the admitted `airs-cli-darwin-arm64`8.0.0 input (SHA256 `818f0082ca0ebfd84cea4c5f9f4e752f1b2293b42cad118b68581fd8a49c2146`,37,323,304bytes) and its original evidence. This source update does not upgrade that consumer or replay P09/P10. |
| Existing MCP deployment pin | Consumer configuration is reconciled separately. CLI9.0 source does not by itself authorize a deployment/service/version change. |

Historical commit metadata is not a recovered payload or accepted executable.
Before retiring the original repository, preserve the required history/input
capsules, hashes, notices, dependency/toolchain recipes and backup/restore proof,
and remove or deliberately replace remaining old-namespace consumers. GitHub,
release, signing, service/identity and registry writer authority remain held.
