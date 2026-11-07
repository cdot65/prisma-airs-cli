//! The durable administrative authority transitions (contract §3; `authority_rules` in
//! schemas/tools.v1.json). Each public method is one transition of the executable model
//! `airs_runtime_core::authority`, committed as **one** control-store transaction:
//! preview creation, direct-proof execution, approval issuance, execution on the
//! approval path, upload admission and its continuations, and the separately
//! authorized import. Consumption and job insertion commit together or not at all.
//!
//! The MCP tools that reach these transitions arrive in M5; the domain is M3's.
//!
//! Time is epoch milliseconds; a proof's `auth_time` is JWT seconds. Previews and
//! idempotency bindings are scoped to the active recovery epoch: a preview minted
//! before a restore can never be consumed after it.

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::{Value, json};

pub use airs_runtime_core::authority::{Actor, Denied, Proof};
use airs_runtime_core::dto::ErrorCode;

use crate::control::{Control, Refused, canonical};

/// Preview lifetime (contract §3 table: 300 s).
pub const PREVIEW_TTL_MS: i64 = 300_000;
/// Upload session lifetime (24 h).
pub const UPLOAD_TTL_MS: i64 = 86_400_000;
/// Lifecycle jobs default to a one-hour deadline.
pub const LIFECYCLE_DEADLINE_MS: i64 = 3_600_000;
const UPLOAD_ADMIT: &str = "upload_admit";
const IMPORT_FINALIZE: &str = "import_finalize";
const EXECUTE: &str = "lifecycle_execute";
const UPLOAD_BEGIN: &str = "transfer_upload_begin";
/// Actions a `lifecycle_preview` may name (frozen schema enum).
const ACTIONS: &[&str] = &[
    "prune",
    "artifact_delete",
    "compact",
    "backup",
    "restore",
    "recover",
    "import_finalize",
    "export",
    "allowance_set",
    "epoch_activate",
    "upload_admit",
];

/// A transition's refusal: a model denial (state unchanged) or a storage failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityError {
    Denied(Denied),
    Store(Refused),
}

impl From<Denied> for AuthorityError {
    fn from(d: Denied) -> Self {
        Self::Denied(d)
    }
}

impl From<Refused> for AuthorityError {
    fn from(r: Refused) -> Self {
        Self::Store(r)
    }
}

impl AuthorityError {
    /// The wire code (contract §4.2 vocabulary).
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Store(r) => r.code,
            Self::Denied(Denied::NotFound) => ErrorCode::NotFound,
            Self::Denied(Denied::IdempotencyConflict) => ErrorCode::IdempotencyConflict,
            Self::Denied(
                Denied::ActorMismatch | Denied::ProofMissingOrStale | Denied::ApprovalInvalid,
            ) => ErrorCode::Forbidden,
            Self::Denied(Denied::UploadTooLarge | Denied::UploadOffset) => {
                ErrorCode::InvalidRequest
            }
            Self::Denied(_) => ErrorCode::Conflict,
        }
    }
}

type Result<T> = std::result::Result<T, AuthorityError>;

fn store(_: rusqlite::Error) -> AuthorityError {
    AuthorityError::Store(Refused {
        code: ErrorCode::StorageUnavailable,
        reason: "control store unavailable",
        retry_after_seconds: None,
    })
}

fn no_epoch() -> AuthorityError {
    AuthorityError::Store(Refused {
        code: ErrorCode::ControlRestored,
        reason: "no active recovery epoch",
        retry_after_seconds: None,
    })
}

/// A preview as returned to the Administrator: its ID, server nonce and expiry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    pub preview_id: String,
    pub nonce: String,
    pub expires_ms: i64,
    /// The digest of the stored canonical parameters, which execution must present.
    pub digest: String,
}

/// One authority-class submission's bound inputs.
pub struct Attempt<'a> {
    pub tenant: &'a str,
    pub actor: &'a Actor,
    pub preview_id: &'a str,
    pub digest: &'a str,
    /// The archive's current data generation.
    pub generation: i64,
    pub key: &'a str,
    pub expected_epoch: &'a str,
    pub config_revision: &'a str,
    /// The same queue bounds an ordinary submission is admitted under (§6).
    pub queue_tenant: u64,
    pub queue_principal: u64,
}

#[derive(Debug, Clone)]
struct PreviewRow {
    epoch: String,
    action: String,
    actor: Actor,
    digest: String,
    /// The canonical action parameters (the digest is theirs).
    params: Value,
    generation: i64,
    nonce: String,
    expires_ms: i64,
    state: String,
    approval_id: Option<String>,
    upload_id: Option<String>,
}

fn simple_uuid() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn active_epoch(tx: &Transaction<'_>) -> Result<String> {
    tx.query_row("SELECT epoch FROM epochs WHERE state='active'", [], |r| {
        r.get(0)
    })
    .optional()
    .map_err(store)?
    .ok_or_else(no_epoch)
}

fn preview_row(tx: &Transaction<'_>, tenant: &str, id: &str) -> Result<PreviewRow> {
    tx.query_row(
        "SELECT epoch,action,sub,client_id,digest,generation,nonce,expires_ms,state,approval_id,upload_id,params_json FROM previews WHERE tenant=?1 AND preview_id=?2",
        params![tenant, id],
        |r| {
            Ok(PreviewRow {
                epoch: r.get(0)?,
                action: r.get(1)?,
                actor: Actor {
                    sub: r.get(2)?,
                    client_id: r.get(3)?,
                },
                digest: r.get(4)?,
                generation: r.get(5)?,
                nonce: r.get(6)?,
                expires_ms: r.get(7)?,
                state: r.get(8)?,
                approval_id: r.get(9)?,
                upload_id: r.get(10)?,
                params: serde_json::from_str(&r.get::<_, String>(11)?).unwrap_or(Value::Null),
            })
        },
    )
    .optional()
    .map_err(store)?
    .ok_or(AuthorityError::Denied(Denied::NotFound))
}

/// Unexpired, same digest and same data generation (model `live`).
fn live(p: &PreviewRow, digest: &str, generation: i64, now_ms: i64) -> Result<()> {
    if now_ms >= p.expires_ms {
        return Err(Denied::Expired.into());
    }
    if p.digest != digest {
        return Err(Denied::DigestMismatch.into());
    }
    if p.generation != generation {
        return Err(Denied::GenerationChanged.into());
    }
    Ok(())
}

/// The proof carries this preview's nonce and an `auth_time` no older than the limit
/// (and not in the future).
fn proof_ok(p: &PreviewRow, proof: Option<&Proof>, now_ms: i64, max_age_s: u64) -> bool {
    let now_s = (now_ms / 1000).max(0) as u64;
    proof.is_some_and(|f| {
        f.nonce == p.nonce && f.auth_time <= now_s && now_s - f.auth_time <= max_age_s
    })
}

/// The upload an import would consume (model `importable`): owned, unexpired and
/// finalized. Rechecked inside the consuming transition.
fn importable(
    tx: &Transaction<'_>,
    tenant: &str,
    actor: &Actor,
    upload_id: &str,
    now_ms: i64,
) -> Result<()> {
    let (sub, client, expires, state): (String, String, i64, String) = tx
        .query_row(
            "SELECT sub,client_id,expires_ms,state FROM upload_sessions WHERE tenant=?1 AND upload_id=?2",
            params![tenant, upload_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(store)?
        .ok_or(AuthorityError::Denied(Denied::NotFound))?;
    if sub != actor.sub || client != actor.client_id {
        return Err(Denied::ActorMismatch.into());
    }
    if now_ms >= expires {
        return Err(Denied::Expired.into());
    }
    match state.as_str() {
        "finalized" => Ok(()),
        "imported" => Err(Denied::AlreadyConsumed.into()),
        "aborted" => Err(Denied::Expired.into()),
        _ => Err(Denied::UploadNotFinalized.into()),
    }
}

/// Idempotent replay (model `replay`): the identical binding returns the original
/// result; any difference is `idempotency_conflict`.
fn replay(
    tx: &Transaction<'_>,
    a: &Attempt<'_>,
    epoch: &str,
    operation: &str,
    approval: Option<&str>,
) -> Result<Option<String>> {
    let prior: Option<(String, String, i64, Option<String>, String)> = tx
        .query_row(
            "SELECT preview_id,digest,generation,approval_id,result_id FROM authority_keys WHERE tenant=?1 AND sub=?2 AND client_id=?3 AND operation=?4 AND epoch=?5 AND key=?6",
            params![a.tenant, a.actor.sub, a.actor.client_id, operation, epoch, a.key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()
        .map_err(store)?;
    let Some((preview, digest, generation, prior_approval, result)) = prior else {
        return Ok(None);
    };
    // A keyed retry is resolved against its original admitted binding (M5 review 1
    // finding 4): the generation was checked when the key was first bound, and the
    // admitted operation may itself have changed the archive since, so the caller's
    // current generation is not part of the retry's identity.
    let _ = generation;
    if preview == a.preview_id && digest == a.digest && prior_approval.as_deref() == approval {
        Ok(Some(result))
    } else {
        Err(Denied::IdempotencyConflict.into())
    }
}

fn bind_key(
    tx: &Transaction<'_>,
    a: &Attempt<'_>,
    epoch: &str,
    operation: &str,
    approval: Option<&str>,
    result: &str,
) -> Result<()> {
    tx.execute(
        "INSERT INTO authority_keys VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![
            a.tenant,
            a.actor.sub,
            a.actor.client_id,
            operation,
            epoch,
            a.key,
            a.preview_id,
            a.digest,
            a.generation,
            approval,
            result
        ],
    )
    .map_err(store)?;
    Ok(())
}

/// A valid `upload_admit` declaration (frozen `lifecycle_preview.params.upload`): every
/// field present, of its exact shape, and nothing else. Returns `(total_bytes, sha256)`.
fn upload_declaration(params: &Value) -> Option<(u64, String)> {
    let u = params.get("upload")?.as_object()?;
    let hex64 = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    let keys = [
        "expect_environment",
        "expect_tsg",
        "kind",
        "sha256",
        "total_bytes",
    ];
    if u.len() != keys.len() || !keys.iter().all(|k| u.contains_key(*k)) {
        return None;
    }
    let total = u["total_bytes"]
        .as_u64()
        .filter(|t| (1..=17_179_869_184).contains(t))?;
    let sha = u["sha256"].as_str().filter(|s| hex64(s))?;
    let tsg = u["expect_tsg"].as_str()?;
    let env = u["expect_environment"].as_str()?.strip_prefix("env1:")?;
    let kind = u["kind"].as_str()?;
    (!tsg.is_empty()
        && tsg.len() <= 32
        && tsg.bytes().all(|b| b.is_ascii_digit())
        && hex64(env)
        && ["logical_transfer", "legacy_bundle"].contains(&kind))
    .then(|| (total, sha.to_owned()))
}

/// The epoch the caller minted its key under must be the active one.
fn check_epoch(tx: &Transaction<'_>, a: &Attempt<'_>) -> Result<String> {
    let epoch = active_epoch(tx)?;
    if a.expected_epoch != epoch {
        return Err(AuthorityError::Store(Refused {
            code: ErrorCode::ControlRestored,
            reason: "the idempotency key was minted under another recovery epoch",
            retry_after_seconds: None,
        }));
    }
    Ok(epoch)
}

/// Consume the preview (and, for imports, its upload), insert the queued job and bind
/// the key: the single effect of both execute paths (model `admit_job`).
fn admit_job(
    tx: &Transaction<'_>,
    a: &Attempt<'_>,
    epoch: &str,
    p: &PreviewRow,
    approval: Option<&str>,
    now_ms: i64,
) -> Result<String> {
    // The stored parameters must still be the ones the digest names.
    if canonical(&p.params).1 != p.digest {
        return Err(AuthorityError::Store(Refused {
            code: ErrorCode::StorageUnavailable,
            reason: "the preview's stored parameters do not match its digest",
            retry_after_seconds: None,
        }));
    }
    // The same transactional queue admission as an ordinary submission (§6). A refusal
    // rolls back the whole transition: the preview, approval and upload stay unconsumed.
    Control::admit_queue(
        tx,
        a.tenant,
        &crate::control::Actor::Principal {
            sub: a.actor.sub.clone(),
            client_id: a.actor.client_id.clone(),
        },
        a.queue_tenant,
        a.queue_principal,
    )
    .map_err(AuthorityError::Store)?;
    let job_id = format!("j1_{epoch}_{}", uuid::Uuid::new_v4());
    let params_json = json!({
        "action": p.action,
        "action_params": p.params,
        "preview_id": a.preview_id,
        "parameter_digest": a.digest,
        "data_generation": a.generation,
        "approval_id": approval,
        "upload_id": p.upload_id,
    });
    let (canonical_json, digest) = canonical(&params_json);
    tx.execute(
        "INSERT INTO jobs (job_id,epoch,tenant,operation,submitter_sub,submitter_client_id,service_actor,state,submitted_ms,deadline_ms,retry_after_ms,config_revision,credential_revision,scheduled_slot,claimed_by,result_json,updated_ms) VALUES (?1,?2,?3,?4,?5,?6,NULL,'queued',?7,?8,NULL,?9,NULL,NULL,NULL,NULL,?7)",
        params![job_id, epoch, a.tenant, EXECUTE, a.actor.sub, a.actor.client_id, now_ms, now_ms + LIFECYCLE_DEADLINE_MS, a.config_revision],
    )
    .map_err(store)?;
    tx.execute(
        "INSERT INTO job_params VALUES (?1, ?2, ?3)",
        params![job_id, canonical_json, digest],
    )
    .map_err(store)?;
    tx.execute(
        "INSERT INTO job_events VALUES (?1, 0, NULL, 'queued', 'submitted', ?2)",
        params![job_id, now_ms],
    )
    .map_err(store)?;
    let changed = tx
        .execute(
            "UPDATE previews SET state='consumed', consumed_job_id=?3 WHERE tenant=?1 AND preview_id=?2 AND state IN ('open','approved')",
            params![a.tenant, a.preview_id, job_id],
        )
        .map_err(store)?;
    if changed != 1 {
        return Err(Denied::AlreadyConsumed.into());
    }
    if let Some(upload) = &p.upload_id {
        let changed = tx
            .execute(
                "UPDATE upload_sessions SET state='imported' WHERE tenant=?1 AND upload_id=?2 AND state='finalized'",
                params![a.tenant, upload],
            )
            .map_err(store)?;
        if changed != 1 {
            return Err(Denied::AlreadyConsumed.into());
        }
    }
    if let Some(q) = approval {
        let changed = tx
            .execute(
                "UPDATE approvals SET consumed=1 WHERE approval_id=?1 AND consumed=0",
                params![q],
            )
            .map_err(store)?;
        if changed != 1 {
            return Err(Denied::ApprovalInvalid.into());
        }
    }
    bind_key(tx, a, epoch, EXECUTE, approval, &job_id)?;
    Ok(job_id)
}

// Test-only failpoint: abort a transition after every write and before commit,
// proving the transition is atomic.
#[cfg(test)]
thread_local! {
    static FAIL_BEFORE_COMMIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn commit(tx: Transaction<'_>) -> Result<()> {
    #[cfg(test)]
    if FAIL_BEFORE_COMMIT.with(|f| f.get()) {
        drop(tx); // rollback
        return Err(store(rusqlite::Error::InvalidQuery));
    }
    tx.commit().map_err(store)
}

impl Control {
    fn authority_tx(&mut self) -> Result<Transaction<'_>> {
        self.conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(store)
    }

    fn insert_preview(
        tx: &Transaction<'_>,
        tenant: &str,
        epoch: &str,
        p: (&Actor, &str, &Value, i64),
        upload: Option<&str>,
        now_ms: i64,
    ) -> Result<Opened> {
        let (actor, action, params, generation) = p;
        let (params_json, digest) = canonical(params);
        let opened = Opened {
            preview_id: format!("p1_{epoch}_{}", uuid::Uuid::new_v4()),
            nonce: simple_uuid(),
            expires_ms: now_ms + PREVIEW_TTL_MS,
            digest: digest.clone(),
        };
        tx.execute(
            "INSERT INTO previews (preview_id,tenant,epoch,action,sub,client_id,digest,params_json,generation,nonce,created_ms,expires_ms,state,approval_id,upload_id,consumed_job_id) VALUES (?1,?2,?3,?4,?5,?6,?7,?13,?8,?9,?10,?11,'open',NULL,?12,NULL)",
            params![opened.preview_id, tenant, epoch, action, actor.sub, actor.client_id, digest, generation, opened.nonce, now_ms, opened.expires_ms, upload, params_json],
        )
        .map_err(store)?;
        Ok(opened)
    }

    /// A preview's stored canonical parameters (tenant-scoped), for the tools that act
    /// on its declaration (uploads).
    pub fn preview_params(&self, tenant: &str, preview_id: &str) -> Result<Value> {
        let text: Option<String> = self
            .conn
            .query_row(
                "SELECT params_json FROM previews WHERE tenant=?1 AND preview_id=?2",
                params![tenant, preview_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(store)?;
        text.and_then(|t| serde_json::from_str(&t).ok())
            .ok_or(AuthorityError::Denied(Denied::NotFound))
    }

    /// When an approval expires.
    pub fn approval_expires(&self, approval_id: &str) -> Result<i64> {
        self.conn
            .query_row(
                "SELECT expires_ms FROM approvals WHERE approval_id=?1",
                [approval_id],
                |r| r.get(0),
            )
            .map_err(store)
    }

    /// `lifecycle_preview` (read-only for the archive) for any action except
    /// `import_finalize`, which must name a finalized upload ([`Control::preview_import`]).
    pub fn preview(
        &mut self,
        tenant: &str,
        actor: &Actor,
        action: &str,
        params: &Value,
        generation: i64,
        now_ms: i64,
    ) -> Result<Opened> {
        if action == IMPORT_FINALIZE || !ACTIONS.contains(&action) {
            return Err(Denied::WrongAction.into());
        }
        if action == UPLOAD_ADMIT && upload_declaration(params).is_none() {
            return Err(AuthorityError::Store(Refused {
                code: ErrorCode::InvalidRequest,
                reason: "an upload_admit preview needs a complete, valid upload declaration",
                retry_after_seconds: None,
            }));
        }
        let tx = self.authority_tx()?;
        let epoch = active_epoch(&tx)?;
        let opened = Self::insert_preview(
            &tx,
            tenant,
            &epoch,
            (actor, action, params, generation),
            None,
            now_ms,
        )?;
        tx.commit().map_err(store)?;
        Ok(opened)
    }

    /// `lifecycle_preview` with `action = import_finalize`: only the upload's owner,
    /// only for an unexpired finalized upload.
    pub fn preview_import(
        &mut self,
        tenant: &str,
        actor: &Actor,
        upload_id: &str,
        params: &Value,
        generation: i64,
        now_ms: i64,
    ) -> Result<Opened> {
        let tx = self.authority_tx()?;
        let epoch = active_epoch(&tx)?;
        importable(&tx, tenant, actor, upload_id, now_ms)?;
        let opened = Self::insert_preview(
            &tx,
            tenant,
            &epoch,
            (actor, IMPORT_FINALIZE, params, generation),
            Some(upload_id),
            now_ms,
        )?;
        tx.commit().map_err(store)?;
        Ok(opened)
    }

    /// `lifecycle_execute` under `direct_proof`: preview consumption, (for imports)
    /// upload consumption, job insertion and the key binding commit together.
    pub fn execute_direct(
        &mut self,
        a: &Attempt<'_>,
        proof: Option<&Proof>,
        max_proof_age_s: u64,
        now_ms: i64,
    ) -> Result<String> {
        let tx = self.authority_tx()?;
        let epoch = check_epoch(&tx, a)?;
        if let Some(job) = replay(&tx, a, &epoch, EXECUTE, None)? {
            return Ok(job);
        }
        let p = preview_row(&tx, a.tenant, a.preview_id)?;
        if p.epoch != epoch {
            return Err(Denied::Expired.into());
        }
        if p.action == UPLOAD_ADMIT {
            return Err(Denied::WrongAction.into());
        }
        if &p.actor != a.actor {
            return Err(Denied::ActorMismatch.into());
        }
        if p.state != "open" {
            return Err(Denied::AlreadyConsumed.into());
        }
        live(&p, a.digest, a.generation, now_ms)?;
        if let Some(u) = &p.upload_id {
            importable(&tx, a.tenant, a.actor, u, now_ms)?;
        }
        if !proof_ok(&p, proof, now_ms, max_proof_age_s) {
            return Err(Denied::ProofMissingOrStale.into());
        }
        let job = admit_job(&tx, a, &epoch, &p, None, now_ms)?;
        commit(tx)?;
        Ok(job)
    }

    /// `admin_confirm`: the administrative identity approves one agent-prepared
    /// preview with its own fresh proof bound to that preview's nonce. The approval is
    /// bound to the preview, digest, generation and agent, and expires with the preview.
    #[allow(clippy::too_many_arguments)] // each argument is a bound field of the transition
    pub fn confirm(
        &mut self,
        tenant: &str,
        admin: &Actor,
        preview_id: &str,
        digest: &str,
        generation: i64,
        proof: Option<&Proof>,
        max_proof_age_s: u64,
        now_ms: i64,
    ) -> Result<String> {
        let tx = self.authority_tx()?;
        let epoch = active_epoch(&tx)?;
        let p = preview_row(&tx, tenant, preview_id)?;
        if p.epoch != epoch {
            return Err(Denied::Expired.into());
        }
        if p.action == UPLOAD_ADMIT || p.action == IMPORT_FINALIZE {
            return Err(Denied::WrongAction.into());
        }
        if &p.actor == admin {
            return Err(Denied::ActorMismatch.into());
        }
        if p.state != "open" {
            return Err(Denied::AlreadyConsumed.into());
        }
        live(&p, digest, generation, now_ms)?;
        if !proof_ok(&p, proof, now_ms, max_proof_age_s) {
            return Err(Denied::ProofMissingOrStale.into());
        }
        let approval = format!("q1_{}", uuid::Uuid::new_v4());
        tx.execute(
            "INSERT INTO approvals VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,0)",
            params![
                approval,
                preview_id,
                admin.sub,
                admin.client_id,
                p.actor.sub,
                p.actor.client_id,
                p.digest,
                p.generation,
                now_ms,
                p.expires_ms
            ],
        )
        .map_err(store)?;
        tx.execute(
            "UPDATE previews SET state='approved', approval_id=?2 WHERE preview_id=?1 AND state='open'",
            params![preview_id, approval],
        )
        .map_err(store)?;
        commit(tx)?;
        Ok(approval)
    }

    /// `lifecycle_execute` on the approval path, by the preparing agent: the approval
    /// and preview are consumed with the job insert.
    pub fn execute_with_approval(
        &mut self,
        a: &Attempt<'_>,
        approval_id: &str,
        now_ms: i64,
    ) -> Result<String> {
        let tx = self.authority_tx()?;
        let epoch = check_epoch(&tx, a)?;
        if let Some(job) = replay(&tx, a, &epoch, EXECUTE, Some(approval_id))? {
            return Ok(job);
        }
        let approval: Option<(String, String, String, i64, i64)> = tx
            .query_row(
                "SELECT preview_id,agent_sub,agent_client_id,consumed,expires_ms FROM approvals WHERE approval_id=?1",
                params![approval_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()
            .map_err(store)?;
        let Some((preview, sub, client, consumed, expires)) = approval else {
            return Err(Denied::ApprovalInvalid.into());
        };
        if preview != a.preview_id
            || sub != a.actor.sub
            || client != a.actor.client_id
            || consumed != 0
            || now_ms >= expires
        {
            return Err(Denied::ApprovalInvalid.into());
        }
        let p = preview_row(&tx, a.tenant, a.preview_id)?;
        if p.epoch != epoch
            || p.state != "approved"
            || p.approval_id.as_deref() != Some(approval_id)
        {
            return Err(Denied::ApprovalInvalid.into());
        }
        live(&p, a.digest, a.generation, now_ms)?;
        let job = admit_job(&tx, a, &epoch, &p, Some(approval_id), now_ms)?;
        commit(tx)?;
        Ok(job)
    }

    /// `transfer_upload_begin`: consumes an `upload_admit` preview with direct proof
    /// and creates a session owned by that actor; idempotent by key, so a lost
    /// response recovers the same upload ID.
    pub fn upload_begin(
        &mut self,
        a: &Attempt<'_>,
        proof: Option<&Proof>,
        max_proof_age_s: u64,
        total: u64,
        now_ms: i64,
    ) -> Result<String> {
        self.upload_begin_within(a, proof, max_proof_age_s, total, u64::MAX, now_ms)
    }

    /// [`Control::upload_begin`] under the tenant's aggregate staging quota (M5 review 1
    /// finding 8): the declared length of every live upload of the tenant (receiving or
    /// finalized, unexpired) plus this one must fit `quota_bytes`, reserved in the same
    /// transaction that creates the session; otherwise `overloaded`, nothing consumed.
    pub fn upload_begin_within(
        &mut self,
        a: &Attempt<'_>,
        proof: Option<&Proof>,
        max_proof_age_s: u64,
        total: u64,
        quota_bytes: u64,
        now_ms: i64,
    ) -> Result<String> {
        let tx = self.authority_tx()?;
        let epoch = check_epoch(&tx, a)?;
        if let Some(upload) = replay(&tx, a, &epoch, UPLOAD_BEGIN, None)? {
            return Ok(upload);
        }
        let p = preview_row(&tx, a.tenant, a.preview_id)?;
        if p.epoch != epoch {
            return Err(Denied::Expired.into());
        }
        if p.action != UPLOAD_ADMIT {
            return Err(Denied::WrongAction.into());
        }
        if &p.actor != a.actor {
            return Err(Denied::ActorMismatch.into());
        }
        if p.state != "open" {
            return Err(Denied::AlreadyConsumed.into());
        }
        live(&p, a.digest, a.generation, now_ms)?;
        if !proof_ok(&p, proof, now_ms, max_proof_age_s) {
            return Err(Denied::ProofMissingOrStale.into());
        }
        // The upload is bound to its preview's declaration (§3): one must exist, valid
        // in every field, and the length begun must be the length declared. Anything
        // else refuses here, before consumption, leaving the preview open.
        match upload_declaration(&p.params) {
            Some((declared, _)) if declared == total => {}
            Some(_) => return Err(Denied::DigestMismatch.into()),
            None => return Err(Denied::WrongAction.into()),
        }
        let reserved = Self::staging_reserved_in(&tx, a.tenant, now_ms)?;
        if (reserved.max(0) as u64).saturating_add(total) > quota_bytes {
            return Err(AuthorityError::Store(Refused {
                code: ErrorCode::Overloaded,
                reason: "the tenant's staging quota is reserved by other uploads",
                retry_after_seconds: Some(60),
            }));
        }
        let total = i64::try_from(total).map_err(|_| Denied::UploadTooLarge)?;
        let upload = format!("u1_{}", uuid::Uuid::new_v4());
        tx.execute(
            "INSERT INTO upload_sessions VALUES (?1,?2,?3,?4,?5,?6,0,?7,?8,'receiving')",
            params![
                upload,
                a.tenant,
                a.actor.sub,
                a.actor.client_id,
                a.preview_id,
                total,
                now_ms,
                now_ms + UPLOAD_TTL_MS
            ],
        )
        .map_err(store)?;
        let changed = tx
            .execute(
                "UPDATE previews SET state='consumed' WHERE preview_id=?1 AND state='open'",
                params![a.preview_id],
            )
            .map_err(store)?;
        if changed != 1 {
            return Err(Denied::AlreadyConsumed.into());
        }
        bind_key(&tx, a, &epoch, UPLOAD_BEGIN, None, &upload)?;
        commit(tx)?;
        Ok(upload)
    }

    /// The owner's live session: (state, received, total).
    fn session(
        tx: &Transaction<'_>,
        tenant: &str,
        actor: &Actor,
        id: &str,
        now_ms: i64,
    ) -> Result<(String, i64, i64)> {
        let (sub, client, expires, state, received, total): (String, String, i64, String, i64, i64) = tx
            .query_row(
                "SELECT sub,client_id,expires_ms,state,received,total FROM upload_sessions WHERE tenant=?1 AND upload_id=?2",
                params![tenant, id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .optional()
            .map_err(store)?
            .ok_or(AuthorityError::Denied(Denied::NotFound))?;
        if sub != actor.sub || client != actor.client_id {
            return Err(Denied::ActorMismatch.into());
        }
        if now_ms >= expires || state == "aborted" {
            return Err(Denied::Expired.into());
        }
        Ok((state, received, total))
    }

    /// `transfer_upload_chunk`: continuation without new proof; contiguous offsets
    /// only, never beyond the declared length. Returns the new received length.
    pub fn upload_chunk(
        &mut self,
        tenant: &str,
        actor: &Actor,
        id: &str,
        offset: u64,
        len: u64,
        now_ms: i64,
    ) -> Result<u64> {
        let tx = self.authority_tx()?;
        let (state, received, total) = Self::session(&tx, tenant, actor, id, now_ms)?;
        if state != "receiving" {
            return Err(Denied::AlreadyConsumed.into());
        }
        if offset != received as u64 {
            return Err(Denied::UploadOffset.into());
        }
        let next = (received as u64)
            .checked_add(len)
            .filter(|n| *n <= total as u64)
            .ok_or(Denied::UploadTooLarge)?;
        tx.execute(
            "UPDATE upload_sessions SET received=?3 WHERE tenant=?1 AND upload_id=?2 AND received=?4",
            params![tenant, id, next as i64, received],
        )
        .map_err(store)?;
        tx.commit().map_err(store)?;
        Ok(next)
    }

    /// `transfer_upload_finalize`: owner only; the complete length and the declared
    /// hash must match. Idempotent once finalized.
    pub fn upload_finalize(
        &mut self,
        tenant: &str,
        actor: &Actor,
        id: &str,
        sha256: &str,
        now_ms: i64,
    ) -> Result<()> {
        let tx = self.authority_tx()?;
        let (state, received, total) = Self::session(&tx, tenant, actor, id, now_ms)?;
        // The finished bytes must have the SHA-256 the admitting preview declared.
        let declared: Option<String> = tx
            .query_row(
                "SELECT p.params_json FROM upload_sessions u JOIN previews p USING(preview_id) WHERE u.tenant=?1 AND u.upload_id=?2",
                params![tenant, id],
                |r| r.get(0),
            )
            .optional()
            .map_err(store)?;
        let declared = declared
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| upload_declaration(&v).map(|d| d.1));
        let hash_matches = declared.as_deref() == Some(sha256);
        match state.as_str() {
            "finalized" => return Ok(()),
            "imported" => return Err(Denied::AlreadyConsumed.into()),
            _ => {}
        }
        if received != total {
            return Err(Denied::UploadIncomplete.into());
        }
        if !hash_matches {
            return Err(Denied::UploadHashMismatch.into());
        }
        tx.execute(
            "UPDATE upload_sessions SET state='finalized' WHERE tenant=?1 AND upload_id=?2 AND state='receiving'",
            params![tenant, id],
        )
        .map_err(store)?;
        tx.commit().map_err(store)?;
        Ok(())
    }

    /// The admission declaration (`upload` object) of an upload.
    pub fn upload_declaration_of(&self, tenant: &str, id: &str) -> Result<Value> {
        let declared: String = self
            .conn
            .query_row(
                "SELECT p.params_json FROM upload_sessions u JOIN previews p USING(preview_id) WHERE u.tenant=?1 AND u.upload_id=?2",
                params![tenant, id],
                |r| r.get(0),
            )
            .optional()
            .map_err(store)?
            .ok_or(AuthorityError::Denied(Denied::NotFound))?;
        serde_json::from_str::<Value>(&declared)
            .ok()
            .filter(|v| upload_declaration(v).is_some())
            .map(|v| v["upload"].clone())
            .ok_or(AuthorityError::Denied(Denied::WrongAction))
    }

    /// The recorded progress of an upload.
    pub fn upload_progress(&self, tenant: &str, id: &str) -> Result<u64> {
        let received: i64 = self
            .conn
            .query_row(
                "SELECT received FROM upload_sessions WHERE tenant=?1 AND upload_id=?2",
                params![tenant, id],
                |r| r.get(0),
            )
            .optional()
            .map_err(store)?
            .ok_or(AuthorityError::Denied(Denied::NotFound))?;
        Ok(received.max(0) as u64)
    }

    /// The persisted expiry of an upload (the replay of a begin returns it).
    pub fn upload_expires(&self, tenant: &str, id: &str) -> Result<i64> {
        self.conn
            .query_row(
                "SELECT expires_ms FROM upload_sessions WHERE tenant=?1 AND upload_id=?2",
                params![tenant, id],
                |r| r.get(0),
            )
            .optional()
            .map_err(store)?
            .ok_or(AuthorityError::Denied(Denied::NotFound))
    }

    /// The owner's view of one chunk before any byte is written (M5 review 1 finding
    /// 6): `Some(received)` when the chunk is the next one (its bytes are written and
    /// synced, then [`Control::upload_chunk`] advances the progress); `None` when it was
    /// already recorded (a retry after a lost response: the caller verifies its bytes);
    /// a gap or an overrun refuses.
    pub fn upload_chunk_state(
        &mut self,
        tenant: &str,
        actor: &Actor,
        id: &str,
        offset: u64,
        len: u64,
        now_ms: i64,
    ) -> Result<Option<u64>> {
        let tx = self.authority_tx()?;
        let (state, received, total) = Self::session(&tx, tenant, actor, id, now_ms)?;
        let end = offset.checked_add(len).ok_or(Denied::UploadTooLarge)?;
        if end > total as u64 {
            return Err(Denied::UploadTooLarge.into());
        }
        if end <= received as u64 {
            return Ok(None);
        }
        if state != "receiving" {
            return Err(Denied::AlreadyConsumed.into());
        }
        if offset != received as u64 {
            return Err(Denied::UploadOffset.into());
        }
        Ok(Some(received as u64))
    }

    /// The owner's finalization request, before any content is read (M5 review 1 finding
    /// 7): `(state, total, declaration)`. The upload must be complete; a finalized
    /// upload replays; an imported or aborted one refuses.
    pub fn upload_finalize_admit(
        &mut self,
        tenant: &str,
        actor: &Actor,
        id: &str,
        now_ms: i64,
    ) -> Result<(String, u64, Value)> {
        let tx = self.authority_tx()?;
        let (state, received, total) = Self::session(&tx, tenant, actor, id, now_ms)?;
        let declared: Option<String> = tx
            .query_row(
                "SELECT p.params_json FROM upload_sessions u JOIN previews p USING(preview_id) WHERE u.tenant=?1 AND u.upload_id=?2",
                params![tenant, id],
                |r| r.get(0),
            )
            .optional()
            .map_err(store)?;
        let declared = declared
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .filter(|v| upload_declaration(v).is_some())
            .ok_or(AuthorityError::Denied(Denied::WrongAction))?;
        match state.as_str() {
            "finalized" => {}
            "imported" => return Err(Denied::AlreadyConsumed.into()),
            _ if received != total => return Err(Denied::UploadIncomplete.into()),
            _ => {}
        }
        Ok((state, total as u64, declared["upload"].clone()))
    }

    /// A completed upload whose content failed validation: it is aborted (never
    /// importable) and its staging is the caller's to remove.
    pub fn upload_reject(&mut self, tenant: &str, actor: &Actor, id: &str) -> Result<()> {
        let tx = self.authority_tx()?;
        let changed = tx
            .execute(
                "UPDATE upload_sessions SET state='aborted' WHERE tenant=?1 AND upload_id=?2 AND sub=?3 AND client_id=?4 AND state='receiving'",
                params![tenant, id, actor.sub, actor.client_id],
            )
            .map_err(store)?;
        if changed != 1 {
            return Err(Denied::AlreadyConsumed.into());
        }
        tx.commit().map_err(store)?;
        Ok(())
    }

    /// The abandoned-upload sweep (M5 review 1 finding 8): every expired upload that was
    /// never imported is aborted, in one transaction; returns their IDs, whose staging
    /// the caller removes (verified owned files only).
    pub fn sweep_expired_uploads(&mut self, tenant: &str, now_ms: i64) -> Result<Vec<String>> {
        let tx = self.authority_tx()?;
        let ids: Vec<String> = tx
            .prepare(
                "SELECT upload_id FROM upload_sessions WHERE tenant=?1 AND expires_ms<=?2 AND state IN ('receiving','finalized') ORDER BY upload_id",
            )
            .map_err(store)?
            .query_map(params![tenant, now_ms], |r| r.get(0))
            .map_err(store)?
            .collect::<rusqlite::Result<_>>()
            .map_err(store)?;
        tx.execute(
            "UPDATE upload_sessions SET state='aborted' WHERE tenant=?1 AND expires_ms<=?2 AND state IN ('receiving','finalized')",
            params![tenant, now_ms],
        )
        .map_err(store)?;
        tx.commit().map_err(store)?;
        Ok(ids)
    }

    /// The staging bytes a tenant's uploads reserve now (M5 review 2 finding 6): open
    /// uploads until they expire (and are swept), consumed (`imported`) and aborted
    /// uploads until their staging is verifiably removed.
    pub fn staging_reserved(&self, tenant: &str, now_ms: i64) -> Result<i64> {
        Self::staging_reserved_in(&self.conn, tenant, now_ms)
    }

    fn staging_reserved_in(conn: &rusqlite::Connection, tenant: &str, now_ms: i64) -> Result<i64> {
        conn.query_row(
            "SELECT coalesce(sum(total),0) FROM upload_sessions WHERE tenant=?1 AND ((state IN ('receiving','finalized') AND expires_ms>?2) OR state IN ('imported','aborted'))",
            params![tenant, now_ms],
            |r| r.get(0),
        )
        .map_err(store)
    }

    /// The uploads whose staging is due for removal: every aborted upload, and every
    /// consumed (`imported`) upload whose consuming job has ended (whatever its outcome).
    /// Swept again until [`Control::upload_removed`] records the verified removal, so a
    /// restart reconciles whatever an earlier sweep left.
    pub fn uploads_to_remove(&self, tenant: &str) -> Result<Vec<String>> {
        self.conn
            .prepare(
                "SELECT u.upload_id FROM upload_sessions u WHERE u.tenant=?1 AND (u.state='aborted' OR (u.state='imported' AND EXISTS(
                   SELECT 1 FROM previews p JOIN jobs j ON j.job_id=p.consumed_job_id
                    WHERE p.upload_id=u.upload_id AND j.state IN ('succeeded','partial','failed','canceled'))))
                 ORDER BY u.upload_id",
            )
            .map_err(store)?
            .query_map(params![tenant], |r| r.get(0))
            .map_err(store)?
            .collect::<rusqlite::Result<_>>()
            .map_err(store)
    }

    /// Record that an aborted or ended upload's staging was verifiably removed: only now
    /// is its quota released.
    pub fn upload_removed(&mut self, tenant: &str, id: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE upload_sessions SET state='removed' WHERE tenant=?1 AND upload_id=?2 AND state IN ('aborted','imported')",
                params![tenant, id],
            )
            .map_err(store)?;
        Ok(())
    }

    /// `transfer_upload_abort`: owner only, never after import.
    pub fn upload_abort(
        &mut self,
        tenant: &str,
        actor: &Actor,
        id: &str,
        now_ms: i64,
    ) -> Result<()> {
        let tx = self.authority_tx()?;
        // An expired upload can still be aborted by its owner (M5 review 1 finding 8):
        // ownership is checked; expiry only stops further chunks and finalization.
        let (sub, client, state): (String, String, String) = tx
            .query_row(
                "SELECT sub,client_id,state FROM upload_sessions WHERE tenant=?1 AND upload_id=?2",
                params![tenant, id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(store)?
            .ok_or(AuthorityError::Denied(Denied::NotFound))?;
        if sub != actor.sub || client != actor.client_id {
            return Err(Denied::ActorMismatch.into());
        }
        let _ = now_ms;
        if state == "imported" || state == "removed" {
            return Err(Denied::AlreadyConsumed.into());
        }
        tx.execute(
            "UPDATE upload_sessions SET state='aborted' WHERE tenant=?1 AND upload_id=?2",
            params![tenant, id],
        )
        .map_err(store)?;
        tx.commit().map_err(store)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TENANT: &str = "svc-test";
    const EPOCH: &str = "0a1b2c3d";
    /// Seconds; the store's clock is `T_MS`.
    const T: u64 = 1_900_000_000;
    const T_MS: i64 = T as i64 * 1000;
    const AGE: u64 = 120;

    struct Dir(std::path::PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn open() -> (Dir, Control) {
        use std::os::unix::fs::DirBuilderExt;
        let base =
            std::env::temp_dir().join(format!("airs-mcp-authority-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&base)
            .unwrap();
        let c = Control::open(&base, TENANT, EPOCH, &[1u8; 32], T_MS).unwrap();
        (Dir(base), c)
    }

    fn actor(c: &str) -> Actor {
        Actor {
            sub: "human-a".into(),
            client_id: c.into(),
        }
    }

    /// Synthetic action parameters number `n`.
    fn params(n: u8) -> Value {
        json!({"test_params": n})
    }

    /// The digest of [`params`]`(n)`: what a preview of those parameters names.
    fn d(n: u8) -> String {
        canonical(&params(n)).1
    }

    /// A valid `upload_admit` declaration of `total` bytes (frozen schema).
    fn decl(total: u64) -> Value {
        json!({"action": "upload_admit", "upload": {
            "kind": "logical_transfer",
            "total_bytes": total,
            "sha256": SHA,
            "expect_tsg": "1234567890",
            "expect_environment": format!("env1:{}", "a".repeat(64)),
        }})
    }

    /// The SHA-256 every test declaration names.
    const SHA: &str = "abababababababababababababababababababababababababababababababab";

    /// The digest of [`decl`]`(total)`.
    fn ud(total: u64) -> String {
        canonical(&decl(total)).1
    }

    /// The parameters whose digest is `digest` (test digests come from [`d`] or [`ud`]).
    fn params_for(digest: &str) -> Value {
        (0..=u8::MAX)
            .map(params)
            .chain([4, 10, 100, 1024].into_iter().map(decl))
            .find(|p| canonical(p).1 == digest)
            .expect("a test digest made by d() or ud()")
    }

    fn proof(n: &str) -> Proof {
        Proof {
            nonce: n.into(),
            auth_time: T,
        }
    }

    fn key(k: &str) -> String {
        format!("key-{k:0>12}")
    }

    fn attempt<'a>(
        actor: &'a Actor,
        preview: &'a str,
        digest: &'a str,
        generation: i64,
        key: &'a str,
    ) -> Attempt<'a> {
        Attempt {
            tenant: TENANT,
            actor,
            preview_id: preview,
            digest,
            generation,
            key,
            expected_epoch: EPOCH,
            config_revision: "rev-1",
            queue_tenant: crate::control::DEFAULT_QUEUE_TENANT,
            queue_principal: crate::control::DEFAULT_QUEUE_PRINCIPAL,
        }
    }

    fn denied<T: std::fmt::Debug>(r: Result<T>) -> Denied {
        match r {
            Err(AuthorityError::Denied(d)) => d,
            other => panic!("expected a denial, got {other:?}"),
        }
    }

    fn count(c: &Control, sql: &str) -> i64 {
        c.conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    fn jobs(c: &Control) -> i64 {
        count(c, "SELECT count(*) FROM jobs")
    }

    /// Model `direct_proof_flow_is_single_use_and_retries_must_repeat_the_exact_binding`.
    #[test]
    fn direct_proof_flow_is_single_use_and_retries_must_repeat_the_exact_binding() {
        let (_d, mut c) = open();
        let admin = actor("airs-admin-cli");
        let (d1, d2) = (d(1), d(2));
        let k1 = key("k1");
        let p = c
            .preview(TENANT, &admin, "prune", &params_for(&d1), 7, T_MS)
            .unwrap();
        assert!(p.preview_id.starts_with(&format!("p1_{EPOCH}_")));
        assert_eq!(p.nonce.len(), 32);
        let pid = p.preview_id.as_str();
        assert_eq!(
            denied(c.execute_direct(&attempt(&admin, pid, &d1, 7, &k1), None, AGE, T_MS)),
            Denied::ProofMissingOrStale
        );
        let stale = Proof {
            nonce: p.nonce.clone(),
            auth_time: T - 600,
        };
        assert_eq!(
            denied(c.execute_direct(&attempt(&admin, pid, &d1, 7, &k1), Some(&stale), AGE, T_MS)),
            Denied::ProofMissingOrStale
        );
        let future = Proof {
            nonce: p.nonce.clone(),
            auth_time: T + 5,
        };
        assert_eq!(
            denied(c.execute_direct(&attempt(&admin, pid, &d1, 7, &k1), Some(&future), AGE, T_MS)),
            Denied::ProofMissingOrStale,
            "an auth_time in the future is not fresh"
        );
        let good = proof(&p.nonce);
        assert_eq!(
            denied(c.execute_direct(&attempt(&admin, pid, &d2, 7, &k1), Some(&good), AGE, T_MS)),
            Denied::DigestMismatch
        );
        assert_eq!(
            denied(c.execute_direct(&attempt(&admin, pid, &d1, 8, &k1), Some(&good), AGE, T_MS)),
            Denied::GenerationChanged
        );
        assert_eq!(jobs(&c), 0, "refusals create nothing");
        let job = c
            .execute_direct(&attempt(&admin, pid, &d1, 7, &k1), Some(&good), AGE, T_MS)
            .unwrap();
        assert!(job.starts_with(&format!("j1_{EPOCH}_")));
        assert_eq!(
            c.execute_direct(
                &attempt(&admin, pid, &d1, 7, &k1),
                Some(&good),
                AGE,
                T_MS + 5_000
            )
            .unwrap(),
            job
        );
        assert_eq!(
            denied(c.execute_direct(&attempt(&admin, pid, &d2, 7, &k1), None, AGE, T_MS + 5_000)),
            Denied::IdempotencyConflict
        );
        // M5 review 1 finding 4: a retry is resolved against its original admitted
        // binding, so a generation that moved since (the admitted job itself may have
        // changed the archive) still returns the original job.
        assert_eq!(
            c.execute_direct(&attempt(&admin, pid, &d1, 9, &k1), None, AGE, T_MS + 5_000)
                .unwrap(),
            job
        );
        let other = c
            .preview(TENANT, &admin, "prune", &params_for(&d1), 7, T_MS)
            .unwrap();
        assert_eq!(
            denied(c.execute_direct(
                &attempt(&admin, &other.preview_id, &d1, 7, &k1),
                None,
                AGE,
                T_MS + 5_000
            )),
            Denied::IdempotencyConflict
        );
        assert_eq!(
            denied(c.execute_direct(
                &attempt(&admin, pid, &d1, 7, &key("k2")),
                Some(&good),
                AGE,
                T_MS + 5_000
            )),
            Denied::AlreadyConsumed
        );
        assert_eq!(jobs(&c), 1);
        // The preview is consumed together with the job, which is queued with its binding.
        let (state, consumed): (String, String) = c
            .conn
            .query_row(
                "SELECT state, consumed_job_id FROM previews WHERE preview_id=?1",
                [pid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            (state.as_str(), consumed.as_str()),
            ("consumed", job.as_str())
        );
        let job_row = c.job(TENANT, &job).unwrap().unwrap();
        assert_eq!(job_row.operation, "lifecycle_execute");
        assert_eq!(job_row.state, crate::jobs::State::Queued);
        let p2 = c
            .preview(TENANT, &admin, "prune", &params_for(&d1), 7, T_MS)
            .unwrap();
        let late_ms = T_MS + PREVIEW_TTL_MS;
        let late = Proof {
            nonce: p2.nonce.clone(),
            auth_time: T + 300,
        };
        assert_eq!(
            denied(c.execute_direct(
                &attempt(&admin, &p2.preview_id, &d1, 7, &key("k3")),
                Some(&late),
                AGE,
                late_ms
            )),
            Denied::Expired
        );
    }

    /// Model `a_proof_is_never_reusable_for_another_preview_actor_or_path`.
    #[test]
    fn a_proof_is_never_reusable_for_another_preview_actor_or_path() {
        let (_d, mut c) = open();
        let admin = actor("airs-admin-cli");
        let other = actor("airs-admin-cli-2");
        let dd = ud(10);
        let a = c
            .preview(TENANT, &admin, "prune", &params_for(&dd), 1, T_MS)
            .unwrap();
        let b = c
            .preview(TENANT, &admin, "compact", &params_for(&dd), 1, T_MS)
            .unwrap();
        assert_eq!(
            denied(c.execute_direct(
                &attempt(&admin, &b.preview_id, &dd, 1, &key("kb")),
                Some(&proof(&a.nonce)),
                AGE,
                T_MS
            )),
            Denied::ProofMissingOrStale,
            "cross-operation reuse"
        );
        assert_eq!(
            denied(c.execute_direct(
                &attempt(&other, &a.preview_id, &dd, 1, &key("ka")),
                Some(&proof(&a.nonce)),
                AGE,
                T_MS
            )),
            Denied::ActorMismatch,
            "foreign actor"
        );
        c.execute_direct(
            &attempt(&admin, &a.preview_id, &dd, 1, &key("ka")),
            Some(&proof(&a.nonce)),
            AGE,
            T_MS,
        )
        .unwrap();
        let u = c
            .preview(TENANT, &admin, "upload_admit", &params_for(&dd), 1, T_MS)
            .unwrap();
        assert_eq!(
            denied(c.execute_direct(
                &attempt(&admin, &u.preview_id, &dd, 1, &key("ku")),
                Some(&proof(&u.nonce)),
                AGE,
                T_MS
            )),
            Denied::WrongAction
        );
        assert_eq!(
            denied(c.confirm(
                TENANT,
                &other,
                &u.preview_id,
                &dd,
                1,
                Some(&proof(&u.nonce)),
                AGE,
                T_MS
            )),
            Denied::WrongAction
        );
        let pr = c
            .preview(TENANT, &admin, "prune", &params_for(&dd), 1, T_MS)
            .unwrap();
        assert_eq!(
            denied(c.upload_begin(
                &attempt(&admin, &pr.preview_id, &dd, 1, &key("kx")),
                Some(&proof(&pr.nonce)),
                AGE,
                10,
                T_MS
            )),
            Denied::WrongAction
        );
        assert_eq!(
            denied(c.preview(TENANT, &admin, "import_finalize", &params_for(&dd), 1, T_MS)),
            Denied::WrongAction
        );
        assert_eq!(
            denied(c.preview(TENANT, &admin, "drop_everything", &params_for(&dd), 1, T_MS)),
            Denied::WrongAction,
            "only frozen actions"
        );
        assert_eq!(jobs(&c), 1);
    }

    /// Model `approval_path_binds_admin_proof_to_one_agent_prepared_preview`.
    #[test]
    fn approval_path_binds_admin_proof_to_one_agent_prepared_preview() {
        let (_d, mut c) = open();
        let admin = actor("airs-admin-cli");
        let agent = actor("claude-code-admin-prep");
        let intruder = actor("other-agent");
        let (dd, dx) = (d(4), d(5));
        let k = key("k");
        let p = c
            .preview(TENANT, &agent, "prune", &params_for(&dd), 3, T_MS)
            .unwrap();
        let pid = p.preview_id.as_str();
        assert_eq!(
            denied(c.execute_with_approval(&attempt(&agent, pid, &dd, 3, &k), "q1_none", T_MS)),
            Denied::ApprovalInvalid
        );
        assert_eq!(
            denied(c.confirm(
                TENANT,
                &agent,
                pid,
                &dd,
                3,
                Some(&proof(&p.nonce)),
                AGE,
                T_MS
            )),
            Denied::ActorMismatch,
            "self-approval"
        );
        assert_eq!(
            denied(c.confirm(TENANT, &admin, pid, &dd, 3, None, AGE, T_MS)),
            Denied::ProofMissingOrStale
        );
        assert_eq!(
            denied(c.confirm(
                TENANT,
                &admin,
                pid,
                &dx,
                3,
                Some(&proof(&p.nonce)),
                AGE,
                T_MS
            )),
            Denied::DigestMismatch
        );
        assert_eq!(count(&c, "SELECT count(*) FROM approvals"), 0);
        let q = c
            .confirm(
                TENANT,
                &admin,
                pid,
                &dd,
                3,
                Some(&proof(&p.nonce)),
                AGE,
                T_MS,
            )
            .unwrap();
        assert!(q.starts_with("q1_"));
        assert_eq!(
            denied(c.confirm(
                TENANT,
                &admin,
                pid,
                &dd,
                3,
                Some(&proof(&p.nonce)),
                AGE,
                T_MS
            )),
            Denied::AlreadyConsumed,
            "one approval per preview; the nonce is spent"
        );
        assert_eq!(
            denied(c.execute_with_approval(&attempt(&intruder, pid, &dd, 3, &k), &q, T_MS)),
            Denied::ApprovalInvalid,
            "approvals do not transfer"
        );
        assert_eq!(
            denied(c.execute_with_approval(&attempt(&agent, pid, &dx, 3, &k), &q, T_MS)),
            Denied::DigestMismatch
        );
        assert_eq!(
            denied(c.execute_with_approval(&attempt(&agent, pid, &dd, 4, &k), &q, T_MS)),
            Denied::GenerationChanged
        );
        let p2 = c
            .preview(TENANT, &agent, "prune", &params_for(&dd), 3, T_MS)
            .unwrap();
        assert_eq!(
            denied(c.execute_with_approval(
                &attempt(&agent, &p2.preview_id, &dd, 3, &key("k2")),
                &q,
                T_MS
            )),
            Denied::ApprovalInvalid,
            "approval for another preview"
        );
        assert_eq!(
            denied(c.execute_direct(
                &attempt(&agent, pid, &dd, 3, &key("kd")),
                Some(&proof(&p.nonce)),
                AGE,
                T_MS
            )),
            Denied::AlreadyConsumed,
            "an approved preview is not directly executable"
        );
        assert_eq!(jobs(&c), 0);
        let job = c
            .execute_with_approval(&attempt(&agent, pid, &dd, 3, &k), &q, T_MS)
            .unwrap();
        assert_eq!(
            c.execute_with_approval(&attempt(&agent, pid, &dd, 3, &k), &q, T_MS + 1_000)
                .unwrap(),
            job,
            "exact retry returns the job"
        );
        assert_eq!(
            denied(c.execute_with_approval(&attempt(&agent, pid, &dx, 3, &k), &q, T_MS + 1_000)),
            Denied::IdempotencyConflict
        );
        // M5 review 1 finding 4: a moved generation does not change a retry's identity.
        assert_eq!(
            c.execute_with_approval(&attempt(&agent, pid, &dd, 4, &k), &q, T_MS + 1_000)
                .unwrap(),
            job
        );
        assert_eq!(
            denied(c.execute_with_approval(
                &attempt(&agent, pid, &dd, 3, &k),
                "q1_other",
                T_MS + 1_000
            )),
            Denied::IdempotencyConflict
        );
        assert_eq!(
            denied(c.execute_with_approval(
                &attempt(&agent, pid, &dd, 3, &key("k3")),
                &q,
                T_MS + 1_000
            )),
            Denied::ApprovalInvalid,
            "single use"
        );
        assert_eq!(jobs(&c), 1);
        assert_eq!(count(&c, "SELECT consumed FROM approvals"), 1);
        // An approval expires with its preview.
        let p3 = c
            .preview(TENANT, &agent, "compact", &params_for(&dd), 3, T_MS)
            .unwrap();
        let q3 = c
            .confirm(
                TENANT,
                &admin,
                &p3.preview_id,
                &dd,
                3,
                Some(&proof(&p3.nonce)),
                AGE,
                T_MS,
            )
            .unwrap();
        assert_eq!(
            denied(c.execute_with_approval(
                &attempt(&agent, &p3.preview_id, &dd, 3, &key("k4")),
                &q3,
                T_MS + PREVIEW_TTL_MS
            )),
            Denied::ApprovalInvalid
        );
        assert_eq!(jobs(&c), 1);
    }

    /// Model `complete_upload_flow_begin_chunks_finalize_then_one_separately_authorized_import`.
    #[test]
    fn complete_upload_flow_begin_chunks_finalize_then_one_separately_authorized_import() {
        let (_d, mut c) = open();
        let admin = actor("airs-admin-cli");
        let other = actor("airs-admin-cli-2");
        let (dd, dx, di) = (ud(100), d(7), d(8));
        let kb = key("kb");
        let p = c
            .preview(TENANT, &admin, "upload_admit", &params_for(&dd), 1, T_MS)
            .unwrap();
        let pid = p.preview_id.as_str();
        assert_eq!(
            denied(c.upload_begin(&attempt(&admin, pid, &dd, 1, &kb), None, AGE, 100, T_MS)),
            Denied::ProofMissingOrStale
        );
        let u = c
            .upload_begin(
                &attempt(&admin, pid, &dd, 1, &kb),
                Some(&proof(&p.nonce)),
                AGE,
                100,
                T_MS,
            )
            .unwrap();
        assert!(u.starts_with("u1_"));
        assert_eq!(
            c.upload_begin(
                &attempt(&admin, pid, &dd, 1, &kb),
                None,
                AGE,
                100,
                T_MS + 1_000
            )
            .unwrap(),
            u,
            "lost response recovers the same upload"
        );
        assert_eq!(
            denied(c.upload_begin(
                &attempt(&admin, pid, &dx, 1, &kb),
                None,
                AGE,
                100,
                T_MS + 1_000
            )),
            Denied::IdempotencyConflict
        );
        assert_eq!(
            denied(c.upload_begin(
                &attempt(&admin, pid, &dd, 1, &key("kb2")),
                Some(&proof(&p.nonce)),
                AGE,
                100,
                T_MS + 1_000
            )),
            Denied::AlreadyConsumed
        );
        assert_eq!(c.upload_chunk(TENANT, &admin, &u, 0, 60, T_MS).unwrap(), 60);
        assert_eq!(
            denied(c.upload_chunk(TENANT, &other, &u, 60, 40, T_MS)),
            Denied::ActorMismatch
        );
        assert_eq!(
            denied(c.upload_chunk(TENANT, &admin, &u, 0, 40, T_MS)),
            Denied::UploadOffset
        );
        assert_eq!(
            denied(c.upload_chunk(TENANT, &admin, &u, 60, 41, T_MS)),
            Denied::UploadTooLarge
        );
        assert_eq!(
            denied(c.upload_finalize(TENANT, &admin, &u, SHA, T_MS)),
            Denied::UploadIncomplete
        );
        assert_eq!(
            denied(c.preview_import(TENANT, &admin, &u, &params_for(&di), 1, T_MS)),
            Denied::UploadNotFinalized
        );
        assert_eq!(
            c.upload_chunk(TENANT, &admin, &u, 60, 40, T_MS).unwrap(),
            100
        );
        assert_eq!(
            denied(c.upload_finalize(TENANT, &other, &u, SHA, T_MS)),
            Denied::ActorMismatch
        );
        assert_eq!(
            denied(c.upload_finalize(TENANT, &admin, &u, &"cd".repeat(32), T_MS)),
            Denied::UploadHashMismatch
        );
        c.upload_finalize(TENANT, &admin, &u, SHA, T_MS).unwrap();
        c.upload_finalize(TENANT, &admin, &u, SHA, T_MS).unwrap();
        assert_eq!(
            denied(c.upload_chunk(TENANT, &admin, &u, 100, 0, T_MS)),
            Denied::AlreadyConsumed
        );
        assert_eq!(
            denied(c.preview_import(TENANT, &other, &u, &params_for(&di), 1, T_MS)),
            Denied::ActorMismatch
        );
        let ip = c
            .preview_import(TENANT, &admin, &u, &params_for(&di), 1, T_MS)
            .unwrap();
        assert_eq!(
            denied(c.execute_direct(
                &attempt(&admin, &ip.preview_id, &di, 1, &key("ki")),
                Some(&proof(&p.nonce)),
                AGE,
                T_MS
            )),
            Denied::ProofMissingOrStale,
            "the upload's proof never carries over"
        );
        let job = c
            .execute_direct(
                &attempt(&admin, &ip.preview_id, &di, 1, &key("ki")),
                Some(&proof(&ip.nonce)),
                AGE,
                T_MS,
            )
            .unwrap();
        assert!(job.starts_with("j1_"));
        assert_eq!(
            denied(c.preview_import(TENANT, &admin, &u, &params_for(&di), 1, T_MS)),
            Denied::AlreadyConsumed,
            "one import per upload"
        );
        assert_eq!(
            denied(c.upload_abort(TENANT, &admin, &u, T_MS)),
            Denied::AlreadyConsumed
        );
        let p2 = c
            .preview(TENANT, &admin, "upload_admit", &decl(10), 1, T_MS)
            .unwrap();
        let u2 = c
            .upload_begin(
                &attempt(&admin, &p2.preview_id, &ud(10), 1, &key("kb3")),
                Some(&proof(&p2.nonce)),
                AGE,
                10,
                T_MS,
            )
            .unwrap();
        assert_eq!(
            denied(c.upload_abort(TENANT, &other, &u2, T_MS)),
            Denied::ActorMismatch
        );
        assert_eq!(
            denied(c.upload_chunk(TENANT, &admin, &u2, 0, 5, T_MS + UPLOAD_TTL_MS)),
            Denied::Expired
        );
        c.upload_abort(TENANT, &admin, &u2, T_MS).unwrap();
        assert_eq!(
            denied(c.upload_chunk(TENANT, &admin, &u2, 0, 5, T_MS)),
            Denied::Expired,
            "aborted session"
        );
        assert_eq!(jobs(&c), 1);
    }

    fn finalized_upload(c: &mut Control, admin: &Actor, k: &str) -> String {
        let dd = ud(10);
        let p = c
            .preview(TENANT, admin, "upload_admit", &params_for(&dd), 1, T_MS)
            .unwrap();
        let u = c
            .upload_begin(
                &attempt(admin, &p.preview_id, &dd, 1, &key(k)),
                Some(&proof(&p.nonce)),
                AGE,
                10,
                T_MS,
            )
            .unwrap();
        c.upload_chunk(TENANT, admin, &u, 0, 10, T_MS).unwrap();
        c.upload_finalize(TENANT, admin, &u, SHA, T_MS).unwrap();
        u
    }

    /// Model `overlapping_import_previews_admit_exactly_one_import`.
    #[test]
    fn overlapping_import_previews_admit_exactly_one_import() {
        let (_d, mut c) = open();
        let admin = actor("airs-admin-cli");
        let di = d(8);
        let u = finalized_upload(&mut c, &admin, "kb");
        let i1 = c
            .preview_import(TENANT, &admin, &u, &params_for(&di), 1, T_MS)
            .unwrap();
        let i2 = c
            .preview_import(TENANT, &admin, &u, &params_for(&di), 1, T_MS)
            .unwrap();
        let job = c
            .execute_direct(
                &attempt(&admin, &i1.preview_id, &di, 1, &key("k1")),
                Some(&proof(&i1.nonce)),
                AGE,
                T_MS,
            )
            .unwrap();
        assert_eq!(
            denied(c.execute_direct(
                &attempt(&admin, &i2.preview_id, &di, 1, &key("k2")),
                Some(&proof(&i2.nonce)),
                AGE,
                T_MS
            )),
            Denied::AlreadyConsumed
        );
        assert_eq!(jobs(&c), 1, "one upload, one import job");
        assert_eq!(
            c.execute_direct(
                &attempt(&admin, &i1.preview_id, &di, 1, &key("k1")),
                None,
                AGE,
                T_MS + 1_000
            )
            .unwrap(),
            job
        );
    }

    /// Model `abort_or_expiry_after_preview_blocks_the_import`.
    #[test]
    fn abort_or_expiry_after_preview_blocks_the_import() {
        let (_d, mut c) = open();
        let admin = actor("airs-admin-cli");
        let di = d(8);
        let u = finalized_upload(&mut c, &admin, "kb");
        let ip = c
            .preview_import(TENANT, &admin, &u, &params_for(&di), 1, T_MS)
            .unwrap();
        c.upload_abort(TENANT, &admin, &u, T_MS).unwrap();
        assert_eq!(
            denied(c.execute_direct(
                &attempt(&admin, &ip.preview_id, &di, 1, &key("k")),
                Some(&proof(&ip.nonce)),
                AGE,
                T_MS
            )),
            Denied::Expired,
            "aborted after preview"
        );
        assert_eq!(jobs(&c), 0);
        let u2 = finalized_upload(&mut c, &admin, "kb2");
        let late = T_MS + UPLOAD_TTL_MS;
        let ip2 = c
            .preview_import(TENANT, &admin, &u2, &params_for(&di), 1, late - 10_000)
            .unwrap();
        assert_eq!(
            denied(c.preview_import(TENANT, &admin, &u2, &params_for(&di), 1, late)),
            Denied::Expired
        );
        let fresh_late = Proof {
            nonce: ip2.nonce.clone(),
            auth_time: T + UPLOAD_TTL_MS as u64 / 1000,
        };
        assert_eq!(
            denied(c.execute_direct(
                &attempt(&admin, &ip2.preview_id, &di, 1, &key("k2")),
                Some(&fresh_late),
                AGE,
                late
            )),
            Denied::Expired
        );
        assert_eq!(jobs(&c), 0);
    }

    /// Atomicity: a job-creating transition that fails after all its writes and
    /// before commit leaves no job, no consumed preview, approval or upload, and no key
    /// binding; the same request then succeeds normally.
    #[test]
    fn consumption_and_job_insertion_commit_together_or_not_at_all() {
        let (_d, mut c) = open();
        let admin = actor("airs-admin-cli");
        let agent = actor("claude-code-admin-prep");
        let dd = ud(4);
        let snapshot = |c: &Control| {
            (
                jobs(c),
                count(c, "SELECT count(*) FROM job_events"),
                count(c, "SELECT count(*) FROM authority_keys"),
                count(c, "SELECT count(*) FROM previews WHERE state='consumed'"),
                count(c, "SELECT count(*) FROM approvals WHERE consumed=1"),
                count(
                    c,
                    "SELECT count(*) FROM upload_sessions WHERE state='imported'",
                ),
            )
        };
        let fail = |on: bool| FAIL_BEFORE_COMMIT.with(|f| f.set(on));

        // Direct execution.
        let p = c
            .preview(TENANT, &admin, "prune", &params_for(&dd), 1, T_MS)
            .unwrap();
        let before = snapshot(&c);
        fail(true);
        let r = c.execute_direct(
            &attempt(&admin, &p.preview_id, &dd, 1, &key("a1")),
            Some(&proof(&p.nonce)),
            AGE,
            T_MS,
        );
        fail(false);
        assert!(matches!(r, Err(AuthorityError::Store(_))));
        assert_eq!(
            snapshot(&c),
            before,
            "nothing of the failed transition remains"
        );
        c.execute_direct(
            &attempt(&admin, &p.preview_id, &dd, 1, &key("a1")),
            Some(&proof(&p.nonce)),
            AGE,
            T_MS,
        )
        .unwrap();

        // Approval issuance and approval-path execution.
        let p = c
            .preview(TENANT, &agent, "compact", &params_for(&dd), 1, T_MS)
            .unwrap();
        let before = snapshot(&c);
        fail(true);
        let r = c.confirm(
            TENANT,
            &admin,
            &p.preview_id,
            &dd,
            1,
            Some(&proof(&p.nonce)),
            AGE,
            T_MS,
        );
        fail(false);
        assert!(r.is_err());
        assert_eq!(count(&c, "SELECT count(*) FROM approvals"), 0);
        assert_eq!(snapshot(&c), before);
        let q = c
            .confirm(
                TENANT,
                &admin,
                &p.preview_id,
                &dd,
                1,
                Some(&proof(&p.nonce)),
                AGE,
                T_MS,
            )
            .unwrap();
        let before = snapshot(&c);
        fail(true);
        let r = c.execute_with_approval(
            &attempt(&agent, &p.preview_id, &dd, 1, &key("a2")),
            &q,
            T_MS,
        );
        fail(false);
        assert!(r.is_err());
        assert_eq!(snapshot(&c), before);
        c.execute_with_approval(
            &attempt(&agent, &p.preview_id, &dd, 1, &key("a2")),
            &q,
            T_MS,
        )
        .unwrap();

        // Upload admission, then the import that consumes the upload.
        let p = c
            .preview(TENANT, &admin, "upload_admit", &params_for(&dd), 1, T_MS)
            .unwrap();
        fail(true);
        let r = c.upload_begin(
            &attempt(&admin, &p.preview_id, &dd, 1, &key("a3")),
            Some(&proof(&p.nonce)),
            AGE,
            4,
            T_MS,
        );
        fail(false);
        assert!(r.is_err());
        assert_eq!(count(&c, "SELECT count(*) FROM upload_sessions"), 0);
        let u = c
            .upload_begin(
                &attempt(&admin, &p.preview_id, &dd, 1, &key("a3")),
                Some(&proof(&p.nonce)),
                AGE,
                4,
                T_MS,
            )
            .unwrap();
        c.upload_chunk(TENANT, &admin, &u, 0, 4, T_MS).unwrap();
        c.upload_finalize(TENANT, &admin, &u, SHA, T_MS).unwrap();
        let ip = c
            .preview_import(TENANT, &admin, &u, &params_for(&dd), 1, T_MS)
            .unwrap();
        let before = snapshot(&c);
        fail(true);
        let r = c.execute_direct(
            &attempt(&admin, &ip.preview_id, &dd, 1, &key("a4")),
            Some(&proof(&ip.nonce)),
            AGE,
            T_MS,
        );
        fail(false);
        assert!(r.is_err());
        assert_eq!(
            snapshot(&c),
            before,
            "the upload stays finalized and importable"
        );
        c.execute_direct(
            &attempt(&admin, &ip.preview_id, &dd, 1, &key("a4")),
            Some(&proof(&ip.nonce)),
            AGE,
            T_MS,
        )
        .unwrap();
        assert_eq!(jobs(&c), 3);
    }

    /// Previews and keys are bound to the active recovery epoch: a key minted under
    /// another epoch is `control_restored` and creates nothing; a store with no
    /// active epoch mints no preview.
    #[test]
    fn authority_transitions_are_bound_to_the_active_epoch() {
        let (_d, mut c) = open();
        let admin = actor("airs-admin-cli");
        let dd = d(10);
        let p = c
            .preview(TENANT, &admin, "prune", &params_for(&dd), 1, T_MS)
            .unwrap();
        let k = key("e1");
        let mut a = attempt(&admin, &p.preview_id, &dd, 1, &k);
        a.expected_epoch = "ffffffff";
        let r = c.execute_direct(&a, Some(&proof(&p.nonce)), AGE, T_MS);
        assert_eq!(r.unwrap_err().code(), ErrorCode::ControlRestored);
        assert_eq!(jobs(&c), 0);
        c.conn
            .execute("UPDATE epochs SET state='retired'", [])
            .unwrap();
        let r = c.preview(TENANT, &admin, "prune", &params_for(&dd), 1, T_MS);
        assert_eq!(r.unwrap_err().code(), ErrorCode::ControlRestored);
    }

    #[test]
    fn denials_map_to_the_fixed_wire_vocabulary() {
        let m = |d: Denied| AuthorityError::Denied(d).code();
        assert_eq!(m(Denied::NotFound), ErrorCode::NotFound);
        assert_eq!(
            m(Denied::IdempotencyConflict),
            ErrorCode::IdempotencyConflict
        );
        assert_eq!(m(Denied::ActorMismatch), ErrorCode::Forbidden);
        assert_eq!(m(Denied::ProofMissingOrStale), ErrorCode::Forbidden);
        assert_eq!(m(Denied::ApprovalInvalid), ErrorCode::Forbidden);
        assert_eq!(m(Denied::DigestMismatch), ErrorCode::Conflict);
        assert_eq!(m(Denied::Expired), ErrorCode::Conflict);
        assert_eq!(m(Denied::UploadOffset), ErrorCode::InvalidRequest);
    }

    /// M3 review 2 finding 4: the action's canonical parameters are stored with the
    /// preview and carried, exactly, into the job they create, so an executor can act
    /// on them after a restart (contract §10 durable inputs). The digest is computed by
    /// the store from those parameters.
    #[test]
    fn executed_jobs_carry_the_previewed_parameters_across_reopen() {
        let (dir, mut c) = open();
        let admin = actor("airs-admin-cli");
        let wanted = json!({"tenant_day": {"sends": 5, "ids": 20}, "reason": "probe"});
        let p = c
            .preview(TENANT, &admin, "allowance_set", &wanted, 3, T_MS)
            .unwrap();
        assert_eq!(p.digest, canonical(&wanted).1);
        let k = key("params");
        let job = c
            .execute_direct(
                &attempt(&admin, &p.preview_id, &p.digest, 3, &k),
                Some(&proof(&p.nonce)),
                AGE,
                T_MS,
            )
            .unwrap();
        drop(c);
        let c = Control::open(&dir.0, TENANT, EPOCH, &[1u8; 32], T_MS).unwrap();
        let stored = c.params(&job).unwrap();
        assert_eq!(stored["action"], "allowance_set");
        assert_eq!(
            stored["action_params"], wanted,
            "the executable inputs survive"
        );
        assert_eq!(stored["parameter_digest"], p.digest.as_str());
    }

    /// M3 review 2 finding 3: authority executions are admitted under the same queue
    /// bounds as ordinary submissions. An over-full queue is `overloaded` with its retry
    /// hint, and the whole transition rolls back: the preview stays open and a later
    /// retry with the same key succeeds once there is room.
    #[test]
    fn authority_jobs_respect_the_queue_bounds_and_leave_previews_unconsumed() {
        let (_d, mut c) = open();
        let admin = actor("airs-admin-cli");
        let mut last = None;
        for n in 0..=(crate::control::DEFAULT_QUEUE_PRINCIPAL as u8) {
            let p = c
                .preview(TENANT, &admin, "compact", &params(n), 1, T_MS)
                .unwrap();
            let k = key(&format!("q{n}"));
            let r = c.execute_direct(
                &attempt(&admin, &p.preview_id, &p.digest, 1, &k),
                Some(&proof(&p.nonce)),
                AGE,
                T_MS,
            );
            last = Some((p, k, r));
        }
        let (p, k, refused) = last.unwrap();
        match refused {
            Err(AuthorityError::Store(r)) => {
                assert_eq!(r.code, ErrorCode::Overloaded);
                assert!(r.retry_after_seconds.is_some(), "the retry hint is kept");
            }
            other => panic!("expected overloaded, got {other:?}"),
        }
        assert_eq!(
            jobs(&c),
            crate::control::DEFAULT_QUEUE_PRINCIPAL as i64,
            "the bound held"
        );
        let state: String = c
            .conn
            .query_row(
                "SELECT state FROM previews WHERE preview_id=?1",
                [&p.preview_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(state, "open", "the refused transition consumed nothing");
        // Room appears (one job finishes); the same key now succeeds.
        let first: String = c
            .conn
            .query_row(
                "SELECT job_id FROM jobs ORDER BY submitted_ms, job_id LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        c.conn
            .execute(
                "UPDATE jobs SET state='succeeded' WHERE job_id=?1",
                [&first],
            )
            .unwrap();
        assert!(
            c.execute_direct(
                &attempt(&admin, &p.preview_id, &p.digest, 1, &k),
                Some(&proof(&p.nonce)),
                AGE,
                T_MS
            )
            .is_ok()
        );
    }

    /// M3 review 2 finding 4 (uploads): an upload begun from an `upload_admit` preview
    /// must have the size that preview declared.
    #[test]
    fn uploads_are_bound_to_their_declared_size() {
        let (_d, mut c) = open();
        let admin = actor("airs-admin-cli");
        let declared = decl(1024);
        let p = c
            .preview(TENANT, &admin, "upload_admit", &declared, 1, T_MS)
            .unwrap();
        let a = attempt(&admin, &p.preview_id, &p.digest, 1, "key-upload-00000001");
        assert_eq!(
            denied(c.upload_begin(&a, Some(&proof(&p.nonce)), AGE, 2048, T_MS)),
            Denied::DigestMismatch
        );
        assert!(
            c.upload_begin(&a, Some(&proof(&p.nonce)), AGE, 1024, T_MS)
                .is_ok()
        );
    }

    /// M3 review 3 finding 2: an `upload_admit` preview needs a complete, valid
    /// declaration, so a proof always authorizes a declared length and hash. A missing
    /// or malformed declaration is refused at preview; an upload begun at another
    /// length is refused with the preview left open; finalize compares the finished
    /// bytes' SHA-256 with the declared one.
    #[test]
    fn uploads_need_a_complete_declaration_and_finish_with_its_hash() {
        let (_d, mut c) = open();
        let admin = actor("airs-admin-cli");
        for bad in [
            json!({}),
            json!({"action": "upload_admit"}),
            json!({"action": "upload_admit", "upload": {"total_bytes": 10}}),
            {
                let mut v = decl(10);
                v["upload"]["sha256"] = json!("XYZ");
                v
            },
            {
                let mut v = decl(10);
                v["upload"]["extra"] = json!(1);
                v
            },
            decl(0),
        ] {
            assert!(
                matches!(
                    c.preview(TENANT, &admin, "upload_admit", &bad, 1, T_MS),
                    Err(AuthorityError::Store(_))
                ),
                "{bad}"
            );
        }
        let p = c
            .preview(TENANT, &admin, "upload_admit", &decl(10), 1, T_MS)
            .unwrap();
        let a = attempt(&admin, &p.preview_id, &p.digest, 1, "key-upload-00000002");
        assert_eq!(
            denied(c.upload_begin(&a, Some(&proof(&p.nonce)), AGE, 11, T_MS)),
            Denied::DigestMismatch
        );
        let state: String = c
            .conn
            .query_row(
                "SELECT state FROM previews WHERE preview_id=?1",
                [&p.preview_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(state, "open", "a refused begin consumes nothing");
        let u = c
            .upload_begin(&a, Some(&proof(&p.nonce)), AGE, 10, T_MS)
            .unwrap();
        c.upload_chunk(TENANT, &admin, &u, 0, 10, T_MS).unwrap();
        assert_eq!(
            denied(c.upload_finalize(TENANT, &admin, &u, &"cd".repeat(32), T_MS)),
            Denied::UploadHashMismatch
        );
        c.upload_finalize(TENANT, &admin, &u, SHA, T_MS).unwrap();
        // Defence in depth: a stored preview whose declaration is missing (as no preview
        // call can create) is refused at begin, and stays unconsumed.
        let q = c
            .preview(TENANT, &admin, "upload_admit", &decl(10), 1, T_MS)
            .unwrap();
        c.conn
            .execute(
                "UPDATE previews SET params_json='{}' WHERE preview_id=?1",
                [&q.preview_id],
            )
            .unwrap();
        let b = attempt(&admin, &q.preview_id, &q.digest, 1, "key-upload-00000003");
        assert_eq!(
            denied(c.upload_begin(&b, Some(&proof(&q.nonce)), AGE, 10, T_MS)),
            Denied::WrongAction
        );
    }
}
