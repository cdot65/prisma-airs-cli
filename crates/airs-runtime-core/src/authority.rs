//! Executable model of the frozen administrative authority transitions
//! (contract §3; `authority_rules` in schemas/tools.v1.json). M3 implements
//! these as control-store transactions; every transition here is the unit that
//! must commit atomically there. The model is deliberately in-memory and small
//! so the rules themselves are testable before any database exists.

use std::collections::HashMap;

/// `(Idira sub, agent client_id)`: the grant/idempotency/audit key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Actor {
    pub sub: String,
    pub client_id: String,
}

/// A signed, forwarded fresh-authentication proof as the server sees it.
#[derive(Debug, Clone)]
pub struct Proof {
    pub nonce: String,
    pub auth_time: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Denied {
    NotFound,
    Expired,
    ActorMismatch,
    DigestMismatch,
    GenerationChanged,
    ProofMissingOrStale,
    AlreadyConsumed,
    ApprovalInvalid,
    UploadOffset,
    UploadTooLarge,
    UploadIncomplete,
    UploadHashMismatch,
    UploadNotFinalized,
    WrongAction,
    /// Same idempotency key, different binding: never treated as the original.
    IdempotencyConflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewState {
    Open,
    Approved,
    Consumed,
}

#[derive(Debug, Clone)]
struct Preview {
    actor: Actor,
    action: String,
    digest: String,
    generation: u64,
    nonce: String,
    expires_at: u64,
    state: PreviewState,
    approval: Option<String>,
    /// For `import_finalize`: the finalized upload it imports.
    upload: Option<String>,
}

#[derive(Debug, Clone)]
struct Approval {
    preview: String,
    agent: Actor,
    expires_at: u64,
    consumed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UploadState {
    Receiving,
    Finalized,
    Imported,
    Aborted,
}

#[derive(Debug, Clone)]
struct Upload {
    actor: Actor,
    total: u64,
    received: u64,
    expires_at: u64,
    state: UploadState,
}

/// The immutable binding an idempotency key admitted. A retry must present the
/// identical binding to receive the original result (contract §4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Binding {
    preview: String,
    digest: String,
    generation: u64,
    approval: Option<String>,
    result: String,
}

pub const PREVIEW_TTL: u64 = 300;
pub const UPLOAD_TTL: u64 = 24 * 3600;
/// Actions that may only be consumed by their dedicated transition.
const UPLOAD_ADMIT: &str = "upload_admit";
const IMPORT_FINALIZE: &str = "import_finalize";

/// In-memory stand-in for the control store's authority tables. Each public
/// method is one transition that M3 commits as a single control transaction.
#[derive(Debug, Default)]
pub struct Control {
    next: u64,
    previews: HashMap<String, Preview>,
    approvals: HashMap<String, Approval>,
    uploads: HashMap<String, Upload>,
    /// (actor, operation, key) → admitted binding, scoped to this epoch.
    keys: HashMap<(Actor, &'static str, String), Binding>,
    pub jobs_created: u64,
    pub max_proof_age: u64,
}

impl Control {
    pub fn new(max_proof_age: u64) -> Self {
        Self {
            max_proof_age,
            ..Default::default()
        }
    }

    fn id(&mut self, prefix: &str) -> String {
        self.next += 1;
        format!("{prefix}{}", self.next)
    }

    fn insert_preview(
        &mut self,
        actor: &Actor,
        action: &str,
        digest: &str,
        generation: u64,
        upload: Option<String>,
        now: u64,
    ) -> (String, String) {
        let id = self.id("p");
        let nonce = format!("n{}-{id}", self.next * 7919);
        self.previews.insert(
            id.clone(),
            Preview {
                actor: actor.clone(),
                action: action.into(),
                digest: digest.into(),
                generation,
                nonce: nonce.clone(),
                expires_at: now + PREVIEW_TTL,
                state: PreviewState::Open,
                approval: None,
                upload,
            },
        );
        (id, nonce)
    }

    /// `lifecycle_preview` (read-only) for any action except `import_finalize`,
    /// which must name a finalized upload (`preview_import`).
    pub fn preview(
        &mut self,
        actor: &Actor,
        action: &str,
        digest: &str,
        generation: u64,
        now: u64,
    ) -> Result<(String, String), Denied> {
        if action == IMPORT_FINALIZE {
            return Err(Denied::WrongAction);
        }
        Ok(self.insert_preview(actor, action, digest, generation, None, now))
    }

    /// `lifecycle_preview` with `action = import_finalize`: only the upload's
    /// owner, only after finalize, and at most one import per upload.
    pub fn preview_import(
        &mut self,
        actor: &Actor,
        upload_id: &str,
        digest: &str,
        generation: u64,
        now: u64,
    ) -> Result<(String, String), Denied> {
        self.importable(actor, upload_id, now)?;
        Ok(self.insert_preview(
            actor,
            IMPORT_FINALIZE,
            digest,
            generation,
            Some(upload_id.into()),
            now,
        ))
    }

    /// The upload an import would consume: owned by `actor`, unexpired,
    /// finalized and not yet imported or aborted. Checked when an import preview
    /// is created AND again inside the transition that consumes the upload, so
    /// overlapping previews, a later abort or expiry can never admit an import.
    fn importable(&self, actor: &Actor, upload_id: &str, now: u64) -> Result<(), Denied> {
        let u = self.uploads.get(upload_id).ok_or(Denied::NotFound)?;
        if &u.actor != actor {
            return Err(Denied::ActorMismatch);
        }
        if now >= u.expires_at {
            return Err(Denied::Expired);
        }
        match u.state {
            UploadState::Finalized => Ok(()),
            UploadState::Imported => Err(Denied::AlreadyConsumed),
            UploadState::Aborted => Err(Denied::Expired),
            UploadState::Receiving => Err(Denied::UploadNotFinalized),
        }
    }

    fn proof_ok(&self, p: &Preview, proof: Option<&Proof>, now: u64) -> bool {
        proof.is_some_and(|f| {
            f.nonce == p.nonce && f.auth_time <= now && now - f.auth_time <= self.max_proof_age
        })
    }

    fn live(p: &Preview, digest: &str, generation: u64, now: u64) -> Result<(), Denied> {
        if now >= p.expires_at {
            return Err(Denied::Expired);
        }
        if p.digest != digest {
            return Err(Denied::DigestMismatch);
        }
        if p.generation != generation {
            return Err(Denied::GenerationChanged);
        }
        Ok(())
    }

    /// Idempotent replay: identical binding → original result; anything else → conflict.
    fn replay(
        &self,
        actor: &Actor,
        op: &'static str,
        key: &str,
        attempt: &Binding,
    ) -> Option<Result<String, Denied>> {
        let b = self.keys.get(&(actor.clone(), op, key.to_owned()))?;
        let same = b.preview == attempt.preview
            && b.digest == attempt.digest
            && b.generation == attempt.generation
            && b.approval == attempt.approval;
        Some(if same {
            Ok(b.result.clone())
        } else {
            Err(Denied::IdempotencyConflict)
        })
    }

    fn admit_job(
        &mut self,
        actor: &Actor,
        preview_id: &str,
        digest: &str,
        generation: u64,
        approval: Option<String>,
        key: &str,
    ) -> String {
        let job = self.id("j");
        let p = self.previews.get_mut(preview_id).expect("present");
        p.state = PreviewState::Consumed;
        if let Some(u) = p.upload.clone() {
            self.uploads.get_mut(&u).expect("present").state = UploadState::Imported;
        }
        let binding = Binding {
            preview: preview_id.into(),
            digest: digest.into(),
            generation,
            approval,
            result: job.clone(),
        };
        self.keys.insert(
            (actor.clone(), "lifecycle_execute", key.to_owned()),
            binding,
        );
        self.jobs_created += 1;
        job
    }

    /// `lifecycle_execute` under `direct_proof`. Preview consumption, (for imports)
    /// upload consumption and job insertion are one transition.
    #[allow(clippy::too_many_arguments)] // each argument is a bound field of the transition
    pub fn execute_direct(
        &mut self,
        actor: &Actor,
        preview_id: &str,
        digest: &str,
        generation: u64,
        proof: Option<&Proof>,
        key: &str,
        now: u64,
    ) -> Result<String, Denied> {
        let attempt = Binding {
            preview: preview_id.into(),
            digest: digest.into(),
            generation,
            approval: None,
            result: String::new(),
        };
        if let Some(r) = self.replay(actor, "lifecycle_execute", key, &attempt) {
            return r;
        }
        let p = self.previews.get(preview_id).ok_or(Denied::NotFound)?;
        if p.action == UPLOAD_ADMIT {
            return Err(Denied::WrongAction); // only transfer_upload_begin consumes it
        }
        if &p.actor != actor {
            return Err(Denied::ActorMismatch);
        }
        if p.state != PreviewState::Open {
            return Err(Denied::AlreadyConsumed);
        }
        Self::live(p, digest, generation, now)?;
        if let Some(u) = p.upload.clone() {
            self.importable(actor, &u, now)?;
        }
        let p = self.previews.get(preview_id).ok_or(Denied::NotFound)?;
        if !self.proof_ok(p, proof, now) {
            return Err(Denied::ProofMissingOrStale);
        }
        Ok(self.admit_job(actor, preview_id, digest, generation, None, key))
    }

    /// `admin_confirm`: the administrative identity approves one agent-prepared
    /// preview with its own fresh proof bound to that preview's nonce.
    pub fn confirm(
        &mut self,
        admin: &Actor,
        preview_id: &str,
        digest: &str,
        generation: u64,
        proof: Option<&Proof>,
        now: u64,
    ) -> Result<String, Denied> {
        let p = self.previews.get(preview_id).ok_or(Denied::NotFound)?;
        if p.action == UPLOAD_ADMIT || p.action == IMPORT_FINALIZE {
            return Err(Denied::WrongAction); // uploads/imports are never agent-prepared
        }
        if &p.actor == admin {
            return Err(Denied::ActorMismatch);
        }
        if p.state != PreviewState::Open {
            return Err(Denied::AlreadyConsumed);
        }
        Self::live(p, digest, generation, now)?;
        if !self.proof_ok(p, proof, now) {
            return Err(Denied::ProofMissingOrStale);
        }
        let (agent, expires_at) = (p.actor.clone(), p.expires_at);
        let approval = self.id("q");
        let p = self.previews.get_mut(preview_id).expect("present");
        p.state = PreviewState::Approved;
        p.approval = Some(approval.clone());
        self.approvals.insert(
            approval.clone(),
            Approval {
                preview: preview_id.into(),
                agent,
                expires_at,
                consumed: false,
            },
        );
        Ok(approval)
    }

    /// `lifecycle_execute` on the approval path, by the preparing agent.
    #[allow(clippy::too_many_arguments)]
    pub fn execute_with_approval(
        &mut self,
        agent: &Actor,
        preview_id: &str,
        digest: &str,
        generation: u64,
        approval_id: &str,
        key: &str,
        now: u64,
    ) -> Result<String, Denied> {
        let attempt = Binding {
            preview: preview_id.into(),
            digest: digest.into(),
            generation,
            approval: Some(approval_id.into()),
            result: String::new(),
        };
        if let Some(r) = self.replay(agent, "lifecycle_execute", key, &attempt) {
            return r;
        }
        let a = self
            .approvals
            .get(approval_id)
            .ok_or(Denied::ApprovalInvalid)?;
        if a.preview != preview_id || &a.agent != agent || a.consumed || now >= a.expires_at {
            return Err(Denied::ApprovalInvalid);
        }
        let p = self.previews.get(preview_id).ok_or(Denied::NotFound)?;
        if p.state != PreviewState::Approved || p.approval.as_deref() != Some(approval_id) {
            return Err(Denied::ApprovalInvalid);
        }
        Self::live(p, digest, generation, now)?;
        self.approvals
            .get_mut(approval_id)
            .expect("present")
            .consumed = true;
        Ok(self.admit_job(
            agent,
            preview_id,
            digest,
            generation,
            Some(approval_id.into()),
            key,
        ))
    }

    /// `transfer_upload_begin`: consumes an `upload_admit` preview with direct
    /// proof; idempotent by key, so a lost response recovers the same upload ID.
    #[allow(clippy::too_many_arguments)]
    pub fn upload_begin(
        &mut self,
        actor: &Actor,
        preview_id: &str,
        digest: &str,
        generation: u64,
        proof: Option<&Proof>,
        total: u64,
        key: &str,
        now: u64,
    ) -> Result<String, Denied> {
        let attempt = Binding {
            preview: preview_id.into(),
            digest: digest.into(),
            generation,
            approval: None,
            result: String::new(),
        };
        if let Some(r) = self.replay(actor, "transfer_upload_begin", key, &attempt) {
            return r;
        }
        let p = self.previews.get(preview_id).ok_or(Denied::NotFound)?;
        if p.action != UPLOAD_ADMIT {
            return Err(Denied::WrongAction);
        }
        if &p.actor != actor {
            return Err(Denied::ActorMismatch);
        }
        if p.state != PreviewState::Open {
            return Err(Denied::AlreadyConsumed);
        }
        Self::live(p, digest, generation, now)?;
        if !self.proof_ok(p, proof, now) {
            return Err(Denied::ProofMissingOrStale);
        }
        self.previews.get_mut(preview_id).expect("present").state = PreviewState::Consumed;
        let id = self.id("u");
        self.uploads.insert(
            id.clone(),
            Upload {
                actor: actor.clone(),
                total,
                received: 0,
                expires_at: now + UPLOAD_TTL,
                state: UploadState::Receiving,
            },
        );
        self.keys.insert(
            (actor.clone(), "transfer_upload_begin", key.to_owned()),
            Binding {
                result: id.clone(),
                ..attempt
            },
        );
        Ok(id)
    }

    fn session(&mut self, actor: &Actor, id: &str, now: u64) -> Result<&mut Upload, Denied> {
        let u = self.uploads.get_mut(id).ok_or(Denied::NotFound)?;
        if &u.actor != actor {
            return Err(Denied::ActorMismatch);
        }
        if now >= u.expires_at || u.state == UploadState::Aborted {
            return Err(Denied::Expired);
        }
        Ok(u)
    }

    /// `transfer_upload_chunk`: continuation of one admitted operation; no new
    /// proof, contiguous offsets only, never beyond the declared length.
    pub fn upload_chunk(
        &mut self,
        actor: &Actor,
        id: &str,
        offset: u64,
        len: u64,
        now: u64,
    ) -> Result<u64, Denied> {
        let u = self.session(actor, id, now)?;
        if u.state != UploadState::Receiving {
            return Err(Denied::AlreadyConsumed);
        }
        if offset != u.received {
            return Err(Denied::UploadOffset);
        }
        if u.received + len > u.total {
            return Err(Denied::UploadTooLarge);
        }
        u.received += len;
        Ok(u.received)
    }

    /// `transfer_upload_finalize`: owner only; complete length and the declared
    /// hash must match. Idempotent once finalized.
    pub fn upload_finalize(
        &mut self,
        actor: &Actor,
        id: &str,
        hash_matches: bool,
        now: u64,
    ) -> Result<(), Denied> {
        let u = self.session(actor, id, now)?;
        match u.state {
            UploadState::Finalized => return Ok(()),
            UploadState::Imported => return Err(Denied::AlreadyConsumed),
            _ => {}
        }
        if u.received != u.total {
            return Err(Denied::UploadIncomplete);
        }
        if !hash_matches {
            return Err(Denied::UploadHashMismatch);
        }
        u.state = UploadState::Finalized;
        Ok(())
    }

    pub fn upload_abort(&mut self, actor: &Actor, id: &str, now: u64) -> Result<(), Denied> {
        let u = self.session(actor, id, now)?;
        if u.state == UploadState::Imported {
            return Err(Denied::AlreadyConsumed);
        }
        u.state = UploadState::Aborted;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(c: &str) -> Actor {
        Actor {
            sub: "human-a".into(),
            client_id: c.into(),
        }
    }

    const T: u64 = 1_000_000;

    fn proof(n: &str) -> Proof {
        Proof {
            nonce: n.into(),
            auth_time: T,
        }
    }

    #[test]
    fn direct_proof_flow_is_single_use_and_retries_must_repeat_the_exact_binding() {
        let mut c = Control::new(120);
        let admin = actor("airs-admin-cli");
        let (p, nonce) = c.preview(&admin, "prune", "d1", 7, T).unwrap();
        assert_eq!(
            c.execute_direct(&admin, &p, "d1", 7, None, "k1", T),
            Err(Denied::ProofMissingOrStale)
        );
        let stale = Proof {
            nonce: nonce.clone(),
            auth_time: T - 600,
        };
        assert_eq!(
            c.execute_direct(&admin, &p, "d1", 7, Some(&stale), "k1", T),
            Err(Denied::ProofMissingOrStale)
        );
        assert_eq!(
            c.execute_direct(&admin, &p, "d2", 7, Some(&proof(&nonce)), "k1", T),
            Err(Denied::DigestMismatch)
        );
        assert_eq!(
            c.execute_direct(&admin, &p, "d1", 8, Some(&proof(&nonce)), "k1", T),
            Err(Denied::GenerationChanged)
        );
        let job = c
            .execute_direct(&admin, &p, "d1", 7, Some(&proof(&nonce)), "k1", T)
            .unwrap();
        assert_eq!(
            c.execute_direct(&admin, &p, "d1", 7, Some(&proof(&nonce)), "k1", T + 5),
            Ok(job.clone())
        );
        // Post-admission retries with a changed binding are conflicts, not the original.
        assert_eq!(
            c.execute_direct(&admin, &p, "d2", 7, None, "k1", T + 5),
            Err(Denied::IdempotencyConflict)
        );
        assert_eq!(
            c.execute_direct(&admin, &p, "d1", 9, None, "k1", T + 5),
            Err(Denied::IdempotencyConflict)
        );
        let (other, _) = c.preview(&admin, "prune", "d1", 7, T).unwrap();
        assert_eq!(
            c.execute_direct(&admin, &other, "d1", 7, None, "k1", T + 5),
            Err(Denied::IdempotencyConflict)
        );
        assert_eq!(
            c.execute_direct(&admin, &p, "d1", 7, Some(&proof(&nonce)), "k2", T + 5),
            Err(Denied::AlreadyConsumed)
        );
        assert_eq!(c.jobs_created, 1);
        let (p2, n2) = c.preview(&admin, "prune", "d1", 7, T).unwrap();
        let late = Proof {
            nonce: n2,
            auth_time: T + PREVIEW_TTL,
        };
        assert_eq!(
            c.execute_direct(&admin, &p2, "d1", 7, Some(&late), "k3", T + PREVIEW_TTL),
            Err(Denied::Expired)
        );
    }

    #[test]
    fn a_proof_is_never_reusable_for_another_preview_actor_or_path() {
        let mut c = Control::new(120);
        let admin = actor("airs-admin-cli");
        let other = actor("airs-admin-cli-2");
        let (a, nonce_a) = c.preview(&admin, "prune", "d", 1, T).unwrap();
        let (b, _) = c.preview(&admin, "compact", "d", 1, T).unwrap();
        assert_eq!(
            c.execute_direct(&admin, &b, "d", 1, Some(&proof(&nonce_a)), "kb", T),
            Err(Denied::ProofMissingOrStale),
            "cross-operation reuse"
        );
        assert_eq!(
            c.execute_direct(&other, &a, "d", 1, Some(&proof(&nonce_a)), "ka", T),
            Err(Denied::ActorMismatch),
            "foreign actor"
        );
        assert!(
            c.execute_direct(&admin, &a, "d", 1, Some(&proof(&nonce_a)), "ka", T)
                .is_ok()
        );
        // Upload challenges are consumed only by upload_begin, never as lifecycle jobs or approvals.
        let (u, un) = c.preview(&admin, "upload_admit", "d", 1, T).unwrap();
        assert_eq!(
            c.execute_direct(&admin, &u, "d", 1, Some(&proof(&un)), "ku", T),
            Err(Denied::WrongAction)
        );
        assert_eq!(
            c.confirm(&admin, &u, "d", 1, Some(&proof(&un)), T),
            Err(Denied::WrongAction)
        );
        let (pr, n) = c.preview(&admin, "prune", "d", 1, T).unwrap();
        assert_eq!(
            c.upload_begin(&admin, &pr, "d", 1, Some(&proof(&n)), 10, "kx", T),
            Err(Denied::WrongAction)
        );
        // import_finalize previews cannot be minted without a finalized upload.
        assert_eq!(
            c.preview(&admin, "import_finalize", "d", 1, T),
            Err(Denied::WrongAction)
        );
    }

    #[test]
    fn approval_path_binds_admin_proof_to_one_agent_prepared_preview() {
        let mut c = Control::new(120);
        let admin = actor("airs-admin-cli");
        let agent = actor("claude-code-admin-prep");
        let intruder = actor("other-agent");
        let (p, nonce) = c.preview(&agent, "prune", "d", 3, T).unwrap();
        assert_eq!(
            c.execute_with_approval(&agent, &p, "d", 3, "q-none", "k", T),
            Err(Denied::ApprovalInvalid)
        );
        assert_eq!(
            c.confirm(&agent, &p, "d", 3, Some(&proof(&nonce)), T),
            Err(Denied::ActorMismatch),
            "self-approval"
        );
        assert_eq!(
            c.confirm(&admin, &p, "d", 3, None, T),
            Err(Denied::ProofMissingOrStale)
        );
        let q = c
            .confirm(&admin, &p, "d", 3, Some(&proof(&nonce)), T)
            .unwrap();
        assert_eq!(
            c.execute_with_approval(&intruder, &p, "d", 3, &q, "k", T),
            Err(Denied::ApprovalInvalid)
        );
        assert_eq!(
            c.execute_with_approval(&agent, &p, "dx", 3, &q, "k", T),
            Err(Denied::DigestMismatch)
        );
        let (p2, _) = c.preview(&agent, "prune", "d", 3, T).unwrap();
        assert_eq!(
            c.execute_with_approval(&agent, &p2, "d", 3, &q, "k2", T),
            Err(Denied::ApprovalInvalid),
            "approval for another preview"
        );
        let job = c
            .execute_with_approval(&agent, &p, "d", 3, &q, "k", T)
            .unwrap();
        assert_eq!(
            c.execute_with_approval(&agent, &p, "d", 3, &q, "k", T + 1),
            Ok(job),
            "exact retry returns the job"
        );
        // Post-admission retries with changed digest, generation or approval conflict.
        assert_eq!(
            c.execute_with_approval(&agent, &p, "dx", 3, &q, "k", T + 1),
            Err(Denied::IdempotencyConflict)
        );
        assert_eq!(
            c.execute_with_approval(&agent, &p, "d", 4, &q, "k", T + 1),
            Err(Denied::IdempotencyConflict)
        );
        assert_eq!(
            c.execute_with_approval(&agent, &p, "d", 3, "q-other", "k", T + 1),
            Err(Denied::IdempotencyConflict)
        );
        assert_eq!(
            c.execute_with_approval(&agent, &p, "d", 3, &q, "k3", T + 1),
            Err(Denied::ApprovalInvalid),
            "single use"
        );
        assert_eq!(c.jobs_created, 1);
    }

    #[test]
    fn complete_upload_flow_begin_chunks_finalize_then_one_separately_authorized_import() {
        let mut c = Control::new(120);
        let admin = actor("airs-admin-cli");
        let other = actor("airs-admin-cli-2");
        let (p, n) = c.preview(&admin, "upload_admit", "d", 1, T).unwrap();
        assert_eq!(
            c.upload_begin(&admin, &p, "d", 1, None, 100, "kb", T),
            Err(Denied::ProofMissingOrStale)
        );
        let u = c
            .upload_begin(&admin, &p, "d", 1, Some(&proof(&n)), 100, "kb", T)
            .unwrap();
        // Lost response: same key + binding recovers the same upload; changes conflict.
        assert_eq!(
            c.upload_begin(&admin, &p, "d", 1, None, 100, "kb", T + 1),
            Ok(u.clone())
        );
        assert_eq!(
            c.upload_begin(&admin, &p, "dx", 1, None, 100, "kb", T + 1),
            Err(Denied::IdempotencyConflict)
        );
        assert_eq!(
            c.upload_begin(&admin, &p, "d", 1, Some(&proof(&n)), 100, "kb2", T + 1),
            Err(Denied::AlreadyConsumed)
        );
        assert_eq!(c.upload_chunk(&admin, &u, 0, 60, T), Ok(60));
        assert_eq!(
            c.upload_chunk(&other, &u, 60, 40, T),
            Err(Denied::ActorMismatch)
        );
        assert_eq!(
            c.upload_chunk(&admin, &u, 0, 40, T),
            Err(Denied::UploadOffset),
            "replayed offset"
        );
        assert_eq!(
            c.upload_chunk(&admin, &u, 60, 41, T),
            Err(Denied::UploadTooLarge)
        );
        assert_eq!(
            c.upload_finalize(&admin, &u, true, T),
            Err(Denied::UploadIncomplete)
        );
        assert_eq!(
            c.preview_import(&admin, &u, "di", 1, T),
            Err(Denied::UploadNotFinalized)
        );
        assert_eq!(c.upload_chunk(&admin, &u, 60, 40, T), Ok(100));
        assert_eq!(
            c.upload_finalize(&other, &u, true, T),
            Err(Denied::ActorMismatch)
        );
        assert_eq!(
            c.upload_finalize(&admin, &u, false, T),
            Err(Denied::UploadHashMismatch)
        );
        assert_eq!(c.upload_finalize(&admin, &u, true, T), Ok(()));
        assert_eq!(
            c.upload_finalize(&admin, &u, true, T),
            Ok(()),
            "idempotent finalize"
        );
        assert_eq!(
            c.upload_chunk(&admin, &u, 100, 0, T),
            Err(Denied::AlreadyConsumed)
        );
        // Import is a separate preview with its own proof; the upload proof does not carry over.
        assert_eq!(
            c.preview_import(&other, &u, "di", 1, T),
            Err(Denied::ActorMismatch)
        );
        let (ip, inonce) = c.preview_import(&admin, &u, "di", 1, T).unwrap();
        assert_eq!(
            c.execute_direct(&admin, &ip, "di", 1, Some(&proof(&n)), "ki", T),
            Err(Denied::ProofMissingOrStale)
        );
        let job = c
            .execute_direct(&admin, &ip, "di", 1, Some(&proof(&inonce)), "ki", T)
            .unwrap();
        assert!(job.starts_with('j'));
        assert_eq!(
            c.preview_import(&admin, &u, "di", 1, T),
            Err(Denied::AlreadyConsumed),
            "one import per upload"
        );
        assert_eq!(c.upload_abort(&admin, &u, T), Err(Denied::AlreadyConsumed));
        // Abort/expiry of another session.
        let (p2, n2) = c.preview(&admin, "upload_admit", "d", 1, T).unwrap();
        let u2 = c
            .upload_begin(&admin, &p2, "d", 1, Some(&proof(&n2)), 10, "kb3", T)
            .unwrap();
        assert_eq!(c.upload_abort(&other, &u2, T), Err(Denied::ActorMismatch));
        assert_eq!(
            c.upload_chunk(&admin, &u2, 0, 5, T + UPLOAD_TTL),
            Err(Denied::Expired)
        );
        assert_eq!(c.upload_abort(&admin, &u2, T), Ok(()));
        assert_eq!(
            c.upload_chunk(&admin, &u2, 0, 5, T),
            Err(Denied::Expired),
            "aborted session"
        );
    }

    fn finalized_upload(c: &mut Control, admin: &Actor, key: &str) -> String {
        let (p, n) = c.preview(admin, "upload_admit", "d", 1, T).unwrap();
        let u = c
            .upload_begin(admin, &p, "d", 1, Some(&proof(&n)), 10, key, T)
            .unwrap();
        c.upload_chunk(admin, &u, 0, 10, T).unwrap();
        c.upload_finalize(admin, &u, true, T).unwrap();
        u
    }

    #[test]
    fn overlapping_import_previews_admit_exactly_one_import() {
        let mut c = Control::new(120);
        let admin = actor("airs-admin-cli");
        let u = finalized_upload(&mut c, &admin, "kb");
        let (i1, n1) = c.preview_import(&admin, &u, "di", 1, T).unwrap();
        let (i2, n2) = c.preview_import(&admin, &u, "di", 1, T).unwrap();
        let job = c
            .execute_direct(&admin, &i1, "di", 1, Some(&proof(&n1)), "k1", T)
            .unwrap();
        assert_eq!(
            c.execute_direct(&admin, &i2, "di", 1, Some(&proof(&n2)), "k2", T),
            Err(Denied::AlreadyConsumed)
        );
        assert_eq!(c.jobs_created, 1, "one upload, one import job");
        // The exact retry of the admitted import still resolves to the same job.
        assert_eq!(
            c.execute_direct(&admin, &i1, "di", 1, None, "k1", T + 1),
            Ok(job)
        );
    }

    #[test]
    fn abort_or_expiry_after_preview_blocks_the_import() {
        let mut c = Control::new(120);
        let admin = actor("airs-admin-cli");
        let u = finalized_upload(&mut c, &admin, "kb");
        let (ip, n) = c.preview_import(&admin, &u, "di", 1, T).unwrap();
        c.upload_abort(&admin, &u, T).unwrap();
        assert_eq!(
            c.execute_direct(&admin, &ip, "di", 1, Some(&proof(&n)), "k", T),
            Err(Denied::Expired),
            "aborted after preview"
        );
        assert_eq!(c.jobs_created, 0);
        // A finalized upload past its expiry gets no preview, and an earlier
        // preview cannot execute against it either.
        let u2 = finalized_upload(&mut c, &admin, "kb2");
        let late = T + UPLOAD_TTL;
        let (ip2, n2) = c.preview_import(&admin, &u2, "di", 1, late - 10).unwrap();
        assert_eq!(
            c.preview_import(&admin, &u2, "di", 1, late),
            Err(Denied::Expired)
        );
        let fresh_late = Proof {
            nonce: n2,
            auth_time: late,
        };
        assert_eq!(
            c.execute_direct(&admin, &ip2, "di", 1, Some(&fresh_late), "k2", late),
            Err(Denied::Expired)
        );
        assert_eq!(c.jobs_created, 0);
    }
}
