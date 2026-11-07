//! The `lifecycle_execute` executor (contract §3, plan M5.3/M5.4): one previewed,
//! authority-consumed administrative action per job, from the exact parameters its
//! preview stored (`action_params`, carried into the job by the authority store).
//!
//! Online actions in this version:
//! - `prune` (`examples_only`): every live excerpt copy for the selected events is
//!   removed, tombstones recorded, projections rebuilt and the file compacted; earlier
//!   exports and external backups are not erased (no forensic claim). The prune is bound
//!   to the archive generation its preview saw, checked inside its write transaction;
//! - `compact` and `recover`: the archive's verified maintenance operations;
//! - `export`: a private logical transfer of the whole archive, read with `export_read`;
//! - `import_finalize`: the staged, validated, finalized upload, by its declared kind:
//!   a `logical_transfer` through the full staged validation of
//!   `transfer::import_file_controlled` in bounded batches (each checkpointed, so a
//!   restarted job resumes), bound to the preview's generation; a `legacy_bundle`
//!   through the operator-bound legacy evidence import.
//!
//! `backup`, `restore` and `epoch_activate` are offline maintenance of the stopped
//! service (`airs-mcp control-*`, contract §9.1); `allowance_set` and non-examples prune
//! scopes are not offered online in this version. Each is a fixed `result_failure`,
//! never a partial effect.
//!
//! `artifact_delete` (M6, moved from M5) deletes report versions: one control
//! transaction advances the reports fence, tombstones each version and its report,
//! removes its rows and moves the latest pointer; the bundle files are then removed
//! crash-safely (recovery completes an interrupted removal).
//!
//! Every transfer path applies the service's complete secret registry (fail closed).

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use airs_runtime_core::dto::ErrorCode;

use crate::run::Shared;
use crate::worker::{Executor, JobContext, Outcome};

/// The administrative maintenance budget (contract §6: 120 s; compaction 600 s).
pub const MAINTENANCE: Duration = Duration::from_secs(120);
pub const COMPACTION: Duration = Duration::from_secs(600);

/// The checkpoint phase of an import in progress.
pub const IMPORTING: &str = "importing";

pub struct Lifecycle {
    pub shared: Arc<Shared>,
}

fn failure(code: ErrorCode, reason: &str) -> Value {
    json!({
        "operation": "lifecycle_execute",
        "committed_facts": 0,
        "uncertain_sends": 0,
        "error": {"schema_version": 1, "code": code, "reason": reason},
    })
}

fn done(action: &str, affected: Value, imported: Option<u64>, tombstones: u64) -> Value {
    json!({
        "operation": "lifecycle_execute",
        "action": action,
        "affected": affected,
        "backup_id": Value::Null,
        "imported_records": imported,
        "partial_import": false,
        "tombstones": tombstones,
    })
}

/// The export file of a job (private, under the data subtree).
pub fn export_path(ctx_root: &std::path::Path, job_id: &str) -> std::path::PathBuf {
    ctx_root.join("exports").join(format!("{job_id}.jsonl"))
}

/// The staged bytes of an upload.
pub fn upload_path(ctx_root: &std::path::Path, upload_id: &str) -> std::path::PathBuf {
    ctx_root.join("staging").join(format!("{upload_id}.part"))
}

/// Every content check of a legacy evidence bundle (bounded read, normalization and
/// privacy projection of every record), without any archive.
pub fn validate_legacy_bundle(
    path: &std::path::Path,
    cloud: &airs_history_store::identity::Cloud,
    secrets: &airs_runtime_core::error::SecretSet,
) -> Result<u64, airs_runtime_core::error::CliError> {
    let original = airs_history_store::repository::read_legacy(path)?;
    airs_history_store::repository::validate_evidence(cloud, &original, secrets)
}

fn generation_changed(e: &airs_runtime_core::error::CliError) -> bool {
    format!("{e:?}").contains(airs_history_store::repository::GENERATION_CHANGED)
}

impl Executor for Lifecycle {
    fn run(&self, ctx: &JobContext<'_>) -> Outcome {
        let live = self.shared.current();
        let Ok(params) = ctx.params() else {
            return Outcome::Failed(failure(
                ErrorCode::StorageUnavailable,
                "stored parameters are unavailable",
            ));
        };
        let Ok(cloud) = crate::tools::cloud(&live.config) else {
            return Outcome::Failed(failure(
                ErrorCode::StorageUnavailable,
                "archive identity unavailable",
            ));
        };
        let action = params["action"].as_str().unwrap_or_default().to_owned();
        let a = &params["action_params"];
        let bound = params["data_generation"].as_i64();
        let root = ctx.history_root().to_path_buf();
        let data = ctx.data_root().to_path_buf();
        let session = scm::Session::new();
        let secrets = match action.as_str() {
            "export" | "import_finalize" => match crate::tools::service_secrets(&live.config) {
                Ok(s) => s,
                Err(_) => {
                    return Outcome::Failed(failure(
                        ErrorCode::CredentialBlocked,
                        "the service's secret registry cannot be loaded; nothing was transferred",
                    ));
                }
            },
            _ => airs_runtime_core::error::SecretSet::default(),
        };
        // An unbound job (every job the authority store admits carries its binding)
        // never runs a destructive action.
        let Some(generation) = bound else {
            return Outcome::Failed(failure(
                ErrorCode::InvalidRequest,
                "the job carries no data generation binding",
            ));
        };
        let stale = || {
            Outcome::Failed(failure(
                ErrorCode::Conflict,
                "the archive changed since the preview; preview again",
            ))
        };
        let current = || airs_history_store::inspect::data_generation(&root, &cloud, &session);
        match action.as_str() {
            "prune" => {
                if a["examples_only"].as_bool() != Some(true) {
                    return Outcome::Failed(failure(
                        ErrorCode::InvalidRequest,
                        "this version prunes examples only",
                    ));
                }
                let Some(before) = a["before"]
                    .as_str()
                    .and_then(airs_runtime_core::dto::utc_ms)
                else {
                    return Outcome::Failed(failure(
                        ErrorCode::InvalidRequest,
                        "the prune needs a before instant",
                    ));
                };
                let apps: Vec<String> = a["app_ids"]
                    .as_array()
                    .map(|v| {
                        v.iter()
                            .filter_map(|x| x.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                match airs_history_store::repository::prune_examples_bound(
                    &root, &cloud, before, &apps, generation, COMPACTION, &session,
                ) {
                    Ok(v) => {
                        let examples = v["examples"].as_u64().unwrap_or(0);
                        let facts = v["event_facts_with_excerpts"].as_u64().unwrap_or(0)
                            + v["receipt_facts_with_excerpts"].as_u64().unwrap_or(0);
                        // The tombstones the erasure actually recorded (receipt-only
                        // excerpts create tombstones without example rows).
                        let tombstones = v["tombstones"].as_u64().unwrap_or(0);
                        Outcome::Succeeded(done(
                            "prune",
                            json!({"examples": examples, "facts": facts}),
                            None,
                            tombstones,
                        ))
                    }
                    Err(e) if generation_changed(&e) => stale(),
                    Err(_) => Outcome::Failed(failure(
                        ErrorCode::StorageUnavailable,
                        "the prune did not complete; the archive is unchanged or fully pruned",
                    )),
                }
            }
            "compact" | "recover" => {
                // Maintenance changes no data; the binding is checked just before it. A
                // recovery is bound to the archive and journal identity its preview saw
                // (the ordinary generation cannot be read while a journal exists).
                let now = if action == "recover" {
                    airs_history_store::recovery_identity(&root, &cloud, MAINTENANCE, &session)
                } else {
                    current()
                };
                if now.ok() != Some(generation) {
                    return stale();
                }
                let result = if action == "compact" {
                    airs_history_store::compact(&root, &cloud, COMPACTION, &session)
                } else {
                    airs_history_store::recover(&root, &cloud, MAINTENANCE, &session)
                };
                match result {
                    Ok(_) => Outcome::Succeeded(done(&action, json!({}), None, 0)),
                    Err(_) => Outcome::Failed(failure(
                        ErrorCode::MaintenanceRequired,
                        "the archive maintenance did not complete",
                    )),
                }
            }
            "export" => {
                let path = export_path(&data, &ctx.job.job_id);
                if std::fs::create_dir_all(path.parent().expect("exports dir")).is_err() {
                    return Outcome::Failed(failure(
                        ErrorCode::StorageUnavailable,
                        "cannot prepare the export directory",
                    ));
                }
                match airs_history_store::transfer::export_file(
                    &root, &cloud, &path, COMPACTION, &session, &secrets,
                ) {
                    Ok(v) => {
                        let records: u64 = v["records"]
                            .as_object()
                            .map(|m| m.values().filter_map(Value::as_u64).sum())
                            .unwrap_or(0);
                        // The result binds the published file's own SHA-256 (streamed, never
                        // read whole), which every `export_read` chunk is checked against.
                        let Ok((sha, bytes)) = crate::tools::stream_sha256(&path) else {
                            return Outcome::Failed(failure(
                                ErrorCode::StorageUnavailable,
                                "the export file cannot be read back",
                            ));
                        };
                        Outcome::Succeeded(json!({
                            "operation": "export",
                            "bytes": bytes,
                            "records": records,
                            "sha256": sha,
                        }))
                    }
                    Err(_) => Outcome::Failed(failure(
                        ErrorCode::StorageUnavailable,
                        "the export did not complete; no artifact was published",
                    )),
                }
            }
            "import_finalize" => {
                let Some(upload) = params["upload_id"].as_str() else {
                    return Outcome::Failed(failure(
                        ErrorCode::InvalidRequest,
                        "the import names no upload",
                    ));
                };
                let tenant = ctx.job.tenant.clone();
                let declared = ctx.storage().control(|c| {
                    c.upload_declaration_of(&tenant, upload)
                        .map_err(|e| e.code())
                });
                let Ok(declared) = declared else {
                    return Outcome::Failed(failure(
                        ErrorCode::InvalidRequest,
                        "the upload's admission declaration is unavailable",
                    ));
                };
                let path = upload_path(&data, upload);
                match declared["kind"].as_str() {
                    Some("logical_transfer") => {
                        self.import_logical(ctx, &root, &cloud, &path, generation, &secrets)
                    }
                    Some("legacy_bundle") => {
                        if current().ok() != Some(generation) {
                            return stale();
                        }
                        let imported = airs_history_store::repository::read_legacy(&path).and_then(
                            |original| {
                                airs_history_store::repository::import_evidence(
                                    &root,
                                    &cloud,
                                    &original,
                                    &secrets,
                                    &Default::default(),
                                    &session,
                                )
                            },
                        );
                        match imported {
                            Ok(summary) => Outcome::Succeeded(done(
                                "import_finalize",
                                json!({"kind": "legacy_bundle"}),
                                Some(summary.processed_records as u64),
                                0,
                            )),
                            Err(_) => Outcome::Failed(failure(
                                ErrorCode::InvalidRequest,
                                "the legacy import did not complete; its committed batches are idempotent and a repeated import completes it",
                            )),
                        }
                    }
                    _ => Outcome::Failed(failure(
                        ErrorCode::InvalidRequest,
                        "the declared upload kind is not supported",
                    )),
                }
            }
            "artifact_delete" => {
                // Report artifacts (M6; moved from M5 by the §5.1 scope reconciliation):
                // the control transaction deletes and tombstones, then the bundles go.
                let ids: Vec<String> = a["artifact_version_ids"]
                    .as_array()
                    .map(|v| {
                        v.iter()
                            .filter_map(|x| x.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                if ids.is_empty() {
                    return Outcome::Failed(failure(
                        ErrorCode::InvalidRequest,
                        "the deletion names no report version",
                    ));
                }
                let bundles = match ctx.report_bundles() {
                    Ok(b) => b,
                    Err(e) => {
                        return Outcome::Failed(failure(
                            e.code,
                            "the report bundle tree is unavailable",
                        ));
                    }
                };
                let (tenant, job) = (ctx.job.tenant.clone(), ctx.job.job_id.clone());
                let now = airs_runtime_core::time::now_ms();
                let deleted = ctx.with_control(|c| c.delete_versions(&tenant, &ids, &job, now));
                let (now_deleted, already, _assessments) = match deleted {
                    Ok(v) => v,
                    Err(e) => {
                        return Outcome::Failed(failure(
                            e.code,
                            "the deletion was refused before any change",
                        ));
                    }
                };
                // Committed: removing the files is idempotent and completed by recovery
                // if this process stops first (a deleted version is never presented).
                if ctx
                    .with_control(|c| bundles.purge_deleted(c, &tenant))
                    .is_err()
                {
                    return Outcome::Failed(failure(
                        ErrorCode::StorageUnavailable,
                        "the versions are deleted; removing their bundle files did not complete and recovery finishes it",
                    ));
                }
                Outcome::Succeeded(json!({
                    "operation": "lifecycle_execute",
                    "action": "artifact_delete",
                    "affected": {"report_versions": now_deleted + already},
                    "backup_id": Value::Null,
                    "imported_records": Value::Null,
                    "partial_import": false,
                    "tombstones": now_deleted,
                }))
            }
            "backup" | "restore" | "epoch_activate" => Outcome::Failed(failure(
                ErrorCode::MaintenanceRequired,
                "this action is offline maintenance of the stopped service (airs-mcp control-*)",
            )),
            _ => Outcome::Failed(failure(
                ErrorCode::InvalidRequest,
                "this action is not offered online in this version",
            )),
        }
    }
}

impl Lifecycle {
    /// A logical transfer import in bounded, checkpointed batches. A restarted job
    /// resumes from its last durable checkpoint; the result distinguishes a refusal
    /// before any change, a partial commit and an uncertain commit.
    fn import_logical(
        &self,
        ctx: &JobContext<'_>,
        root: &std::path::Path,
        cloud: &airs_history_store::identity::Cloud,
        path: &std::path::Path,
        generation: i64,
        secrets: &airs_runtime_core::error::SecretSet,
    ) -> Outcome {
        use airs_history_store::transfer::{
            IMPORT_BATCH_ROWS, ImportCheckpoint, ImportControl, ImportOutcome,
        };
        let resume = ctx
            .resume_point()
            .ok()
            .flatten()
            .and_then(|(phase, state, _)| {
                (phase == IMPORTING)
                    .then(|| serde_json::from_value::<ImportCheckpoint>(state).ok())
                    .flatten()
            });
        // A cancellation observed at a batch checkpoint stops the import before any
        // further batch or the final rebuild (M5 review 2 finding 5).
        let canceled = std::cell::Cell::new(false);
        let operation = ctx.job.job_id.clone();
        let mut on_batch = |c: &ImportCheckpoint| {
            let state = serde_json::to_value(c).unwrap_or(Value::Null);
            match ctx.checkpoint(IMPORTING, &state, Some(&operation)) {
                Ok(false) => Ok(()),
                Ok(true) => {
                    canceled.set(true);
                    Err(airs_history_store::failure(
                        "the import was canceled at a batch checkpoint",
                    ))
                }
                Err(_) => Err(airs_history_store::failure(
                    "the import's progress could not be recorded",
                )),
            }
        };
        let result = airs_history_store::transfer::import_file_controlled(
            root,
            cloud,
            path,
            COMPACTION,
            &scm::Session::new(),
            secrets,
            ImportControl {
                expected_generation: Some(generation),
                batch_rows: IMPORT_BATCH_ROWS,
                resume,
                operation_id: Some(&operation),
                on_batch: &mut on_batch,
            },
        );
        match result {
            Ok(v) => Outcome::Succeeded(json!({
                "operation": "lifecycle_execute",
                "action": "import_finalize",
                "affected": {"kind": "logical_transfer", "batches": v["batches"], "outcome": "committed"},
                "backup_id": Value::Null,
                "imported_records": v["inserted"],
                "partial_import": false,
                "tombstones": 0,
            })),
            Err(e) => {
                let (outcome, checkpoint, code, reason) = match e.outcome.as_ref() {
                    ImportOutcome::RefusedBeforeChange => (
                        "refused_before_change",
                        None,
                        if generation_changed(&e.error) {
                            ErrorCode::Conflict
                        } else {
                            ErrorCode::InvalidRequest
                        },
                        "the import was refused before any change to the archive",
                    ),
                    ImportOutcome::PartiallyCommitted(c) => (
                        "partially_committed",
                        Some(c.clone()),
                        ErrorCode::Conflict,
                        "some import batches committed and the rest did not; the committed batches are idempotent: a new import of the same transfer completes it",
                    ),
                    ImportOutcome::Uncertain(c) => (
                        "uncertain",
                        Some(c.clone()),
                        ErrorCode::StorageUnavailable,
                        "the commit of one import batch has an unknown outcome; a new import of the same transfer completes it idempotently",
                    ),
                };
                let mut v = failure(code, reason);
                v["action"] = json!("import_finalize");
                v["outcome"] = json!(outcome);
                v["partial_import"] = json!(checkpoint.is_some());
                if let Some(c) = checkpoint {
                    v["committed_batches"] = json!(c.batches);
                    v["committed_rows"] = json!(c.rows);
                    v["committed_facts"] = json!(c.inserted);
                }
                if canceled.get() {
                    v["error"]["reason"] = json!(
                        "the import was canceled at a batch checkpoint; its committed batches are idempotent and remain explicitly incomplete"
                    );
                    return Outcome::CanceledWith(v);
                }
                Outcome::Failed(v)
            }
        }
    }
}
