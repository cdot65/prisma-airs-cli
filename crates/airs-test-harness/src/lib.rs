//! Loopback test harness for `airs-cli-rs`: a deterministic AIRS fixture server, a
//! per-test tenant sandbox, a runner that spawns either CLI under the harness
//! invariants, and comparison helpers for TS-vs-Rust parity tests.
//!
//! # Pieces
//!
//! - [`FixtureServer`]: hyper HTTP/1.1 on `127.0.0.1:0` (own thread and runtime).
//!   OAuth at `/oauth/token` (Basic, raw or RFC 6749 form-encoded halves, or form
//!   `client_id`/`client_secret`; issues `mock-token-<tsg>`), management under `/aisec`
//!   (bearer + path-TSG checks, real `offset`/`limit`/`latest` paging over per-TSG
//!   datasets), scanner under `/scan` (`x-pan-token` + HMAC `x-payload-hash`, or a
//!   scanner bearer; management bearers refused), the AI Gateway workspaces list the TS
//!   doctor probes, and `404 {}` for everything else. Every request is recorded
//!   ([`RecordedRequest`], [`AuthIdentity`]); [`Planned`] responses override any
//!   route once ([`FixtureServer::push_once`]) or persistently
//!   ([`FixtureServer::set_default`]), including delays and hangs whose client
//!   disconnects are counted ([`FixtureServer::disconnects`]).
//! - [`Sandbox`]: `home/`, `state/tenants.json`, `configs/`, `store/` (the airs-cli-rs
//!   TOML store, re-derived from the TypeScript files on every write: [`store`]; see
//!   [`Sandbox::ts_only`], [`Sandbox::rust_only`]), `cwd/`, `tmp/`; writes
//!   configs (mode 0400) and the registry (TS `writeTestRegistry` format, mode 0600);
//!   [`FixtureConfig::standard`] points every endpoint key at the server;
//!   every config write is guarded ([`endpoint_guard_violation`]): a JSON object
//!   config must name all [`ENDPOINT_KEYS`] with loopback URLs or the write panics,
//!   unless a Rust-only test opts out with
//!   [`Sandbox::write_config_allowing_external_endpoints`] (after which the runner
//!   refuses to start the TypeScript CLI in that sandbox);
//!   [`Sandbox::snapshot`]/[`Sandbox::assert_unchanged`] prove the CLIs never write
//!   tenant files ([`Sandbox::snapshot_with`] also covers extra paths such as a
//!   registry under `home/`); [`Sandbox::secrets`] feeds the leak check.
//! - [`run`]/[`spawn`]: stdin `/dev/null`, cwd `<sandbox>/cwd`, allowlisted
//!   environment (`AIRS_CLI_CONFIG_DIR=<sandbox>/store` included; [`escape_violation`]
//!   refuses path variables and absolute arguments outside the sandbox), timeout (kill + panic), no death by signal unless expected, no
//!   fixture secret (raw, trimmed, URL-encoded, base64) on stdout/stderr, and no change
//!   to any sandbox file outside `tmp/` ([`Sandbox::full_snapshot`]; opt out with
//!   [`RunOptions::allow_tenant_writes`]). [`RunOptions::close_stdout_after`] closes
//!   the stdout reader early (broken-pipe tests); [`RunOptions::stall_stdout`] never
//!   reads it while the child runs; [`RunOptions::stall_stderr`] reads stderr only once
//!   released, and [`RunOptions::fill_stderr`] also starts the child on an already full
//!   stderr pipe, so its first stderr write blocks.
//! - [`pty`]: [`spawn_pty`] runs a CLI with stdin and stderr on a pseudo-terminal and
//!   drives its prompts (expect/send; Ctrl-C as a signal). [`lock_holder`]: a helper
//!   process holding the airs-cli-rs store lock. [`RunOptions::stdin`] pipes bytes to a
//!   run instead of `/dev/null`. `tenant` commands never build an API client, so the
//!   loopback-endpoint guards skip them ([`is_tenant_command`]); the path guards apply.
//! - [`compare`]: [`normalize`]d request logs, [`json_semantic_eq`] with JS number
//!   normalisation, [`error_line`]/[`http_status`] stderr extraction, and
//!   [`strip_ts_target_defaults`] (the TypeScript-only target schema defaults, D153).
//!
//! # Example
//!
//! ```no_run
//! use airs_test_harness::{
//!     Cli, FixtureServer, Planned, Route, RunOptions, Sandbox, fixtures, normalize_all,
//!     run,
//! };
//! use serde_json::json;
//!
//! let server = FixtureServer::start();
//! let sandbox = Sandbox::new();
//! sandbox.single_tenant(&server, "prod", "100"); // configs/prod.json + active registry
//! server.set_profiles("100", vec![fixtures::profile(1, "alpha", 1)]);
//!
//! // In the root package's tests: Cli::rust(env!("CARGO_BIN_EXE_airs")).
//! let rust = Cli::rust("target/debug/airs");
//! let args = ["runtime", "profiles", "list", "--output", "json"];
//! // Every run also checks that no sandbox file changed (RunOptions::allow_tenant_writes).
//! let out = run(&rust, &sandbox, &args, &RunOptions::new());
//! out.assert_success();
//! assert_eq!(out.json()[0]["profileName"], "alpha");
//! assert_eq!(server.routes(), vec![Route::Token, Route::Profiles]);
//! assert_eq!(normalize_all(&server.requests()).len(), 2);
//!
//! // Failure injection: a one-shot response is used by the next matching request
//! // (`clear_requests` does not drop it; `clear_overrides` does).
//! server.push_once(Route::Profiles, Planned::json(404, &json!({"message": "nope"})));
//! let failed = run(&rust, &sandbox, &args, &RunOptions::new());
//! assert_eq!(failed.code, 1);
//! ```

#![forbid(unsafe_code)]

mod closed_port;
pub mod compare;
pub mod debug_log;
mod encoding;
pub mod fake_uv;
pub mod fixtures;
pub mod lock_holder;
pub mod pty;
mod runner;
mod sandbox;
mod server;
pub mod store;
pub mod withhold;

pub use closed_port::ClosedPort;
pub use compare::{
    NormalizedBody, NormalizedRequest, PROGRAM_NEXT, RUNTIME_BANNER, assert_json_semantic_eq,
    error_line, http_status, js_normalize, json_semantic_eq, normalise_program,
    normalise_tenant_output, normalize, normalize_all, split_outcome_lines, usage_line,
};
pub use encoding::{base64_encode, form_decode, payload_hash, percent_encode};
pub use pty::{PtyRun, PtySession, spawn_pty, spawn_pty_all, spawn_pty_with};
pub use runner::{
    Cli, DEFAULT_TIMEOUT, PATH_FLAGS, PATH_VARIABLES, PROGRAM_NAMES, Run, RunOptions, Running,
    SIGNAL_MARKER_VAR, StdinSource, bulk_command, child_environment, escape_violation,
    escape_violation_within, is_dlp_command, is_gateway_command, is_model_security_command,
    is_redteam_command, is_runtime_command, is_runtime_extras_command, is_supplychain_command,
    is_tenant_command, path_without_programs, restore_file_argument, resume_state_argument, run,
    spawn, upload_file_argument, uv_hook_violation, workspace_root,
};
pub use sandbox::{
    ENDPOINT_KEYS, FixtureConfig, FixtureTenant, RegistryEntry, Sandbox, SecretForm, Snapshot,
    endpoint_guard_violation, env_guard_violation, is_loopback_url, is_secret_key, registry_json,
};
pub use server::{
    AsyncReceipt, AsyncRow, AsyncRule, AuthIdentity, BasicEncoding, Collection, Dl, DlpData,
    FixtureServer, GatewayData, GroupDeleteMode, Gw, ModelSecData, Ms, Multipart, PageStyle, Part,
    Planned, REGISTRY_EXPIRY, REGISTRY_TOKEN, RecordedRequest, RedTeamData, Route, Rt, RuntimeData,
    Rx, Sc, SupplychainData, TOKEN_PREFIX, multipart, paths, sha256_hex,
    validate_auth_token_preview,
};
