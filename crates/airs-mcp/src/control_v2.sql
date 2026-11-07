-- airs-mcp control store (RUNTIME_MCP_CONTRACT §9.1), schema 1. Holds no credentials.
-- DELETE journal, FULL synchronous, secure_delete ON; ≤ 200 records / 256 KiB batches.
CREATE TABLE meta (
    key TEXT PRIMARY KEY CHECK(key IN ('schema','created_ms','service_tenant','quarantine')),
    value TEXT NOT NULL
) STRICT, WITHOUT ROWID;
-- Recovery epochs: exactly one active. A restored or replaced control store gets a new
-- epoch that an Administrator must activate (C11); old idempotency keys are then
-- `control_restored`.
CREATE TABLE epochs (
    epoch TEXT PRIMARY KEY CHECK(length(epoch)=8),
    created_ms INTEGER NOT NULL,
    activated_ms INTEGER,
    reason TEXT NOT NULL CHECK(reason IN ('initial','restore','replacement')),
    cursor_key BLOB NOT NULL CHECK(length(cursor_key)=32),
    state TEXT NOT NULL CHECK(state IN ('active','pending_activation','retired'))
) STRICT, WITHOUT ROWID;
CREATE UNIQUE INDEX one_active_epoch ON epochs(state) WHERE state='active';
CREATE TABLE jobs (
    job_id TEXT PRIMARY KEY,
    epoch TEXT NOT NULL REFERENCES epochs(epoch),
    tenant TEXT NOT NULL,
    operation TEXT NOT NULL,
    submitter_sub TEXT,
    submitter_client_id TEXT,
    service_actor TEXT,
    state TEXT NOT NULL CHECK(state IN ('queued','running','waiting_retry','needs_credentials','blocked_policy','cancel_requested','canceled','succeeded','partial','failed')),
    submitted_ms INTEGER NOT NULL,
    deadline_ms INTEGER NOT NULL,
    retry_after_ms INTEGER,
    config_revision TEXT NOT NULL,
    credential_revision TEXT,
    scheduled_slot TEXT,
    claimed_by TEXT,
    result_json TEXT CHECK(result_json IS NULL OR json_valid(result_json)),
    gaps_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(gaps_json) AND json_type(gaps_json)='array'),
    updated_ms INTEGER NOT NULL,
    CHECK((submitter_sub IS NULL) = (submitter_client_id IS NULL)),
    CHECK((submitter_sub IS NULL) <> (service_actor IS NULL))
) STRICT, WITHOUT ROWID;
CREATE INDEX jobs_by_state ON jobs(tenant, state, submitted_ms);
CREATE INDEX jobs_by_submitter ON jobs(tenant, submitter_sub, submitter_client_id, submitted_ms);
CREATE TABLE job_params (
    job_id TEXT PRIMARY KEY REFERENCES jobs(job_id),
    canonical_json TEXT NOT NULL CHECK(json_valid(canonical_json)),
    digest TEXT NOT NULL CHECK(length(digest)=64)
) STRICT, WITHOUT ROWID;
CREATE TABLE job_checkpoints (
    job_id TEXT NOT NULL REFERENCES jobs(job_id),
    sequence INTEGER NOT NULL CHECK(sequence >= 0),
    phase TEXT NOT NULL,
    state_json TEXT NOT NULL CHECK(json_valid(state_json)),
    archive_operation_id TEXT,
    created_ms INTEGER NOT NULL,
    PRIMARY KEY(job_id, sequence)
) STRICT, WITHOUT ROWID;
CREATE TABLE job_events (
    job_id TEXT NOT NULL REFERENCES jobs(job_id),
    sequence INTEGER NOT NULL CHECK(sequence >= 0),
    from_state TEXT,
    to_state TEXT NOT NULL,
    reason TEXT NOT NULL,
    at_ms INTEGER NOT NULL,
    PRIMARY KEY(job_id, sequence)
) STRICT, WITHOUT ROWID;
CREATE TABLE idempotency (
    tenant TEXT NOT NULL,
    sub TEXT NOT NULL,
    client_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    epoch TEXT NOT NULL REFERENCES epochs(epoch),
    key TEXT NOT NULL CHECK(length(key) BETWEEN 16 AND 64),
    job_id TEXT NOT NULL REFERENCES jobs(job_id),
    digest TEXT NOT NULL CHECK(length(digest)=64),
    PRIMARY KEY(tenant, sub, client_id, operation, epoch, key)
) STRICT, WITHOUT ROWID;
CREATE TABLE send_reservations (
    transport_id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES jobs(job_id),
    source TEXT NOT NULL,
    plane TEXT NOT NULL CHECK(plane IN ('management','scanner','reports','token')),
    credential_revision TEXT,
    state TEXT NOT NULL CHECK(state IN ('reserved','sent','uncertain','not_sent')),
    reserved_ms INTEGER NOT NULL,
    updated_ms INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
CREATE INDEX reservations_by_job ON send_reservations(job_id, state);
CREATE TABLE allowance_days (
    tenant TEXT NOT NULL,
    chicago_date TEXT NOT NULL,
    sends INTEGER NOT NULL CHECK(sends >= 0),
    ids INTEGER NOT NULL CHECK(ids >= 0),
    PRIMARY KEY(tenant, chicago_date)
) STRICT, WITHOUT ROWID;
CREATE TABLE allowance_high_water (
    tenant TEXT PRIMARY KEY,
    high_water_ms INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
-- Jobs whose consumption a recovery cannot know (restore or loss, C11): their budget is
-- spent. They never reserve another send and fail at their next claim with the gap
-- recorded; neither reconciliation nor cooldown expiry removes the mark.
CREATE TABLE exhausted_jobs (
    job_id TEXT PRIMARY KEY REFERENCES jobs(job_id),
    reason TEXT NOT NULL,
    since_ms INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
CREATE TABLE cooldowns (
    tenant TEXT NOT NULL,
    plane TEXT NOT NULL,
    application TEXT NOT NULL DEFAULT '',
    until_ms INTEGER NOT NULL,
    reason TEXT NOT NULL,
    PRIMARY KEY(tenant, plane, application)
) STRICT, WITHOUT ROWID;
CREATE TABLE credential_denials (
    tenant TEXT NOT NULL,
    plane TEXT NOT NULL,
    application TEXT NOT NULL DEFAULT '',
    credential_revision TEXT NOT NULL,
    denied_ms INTEGER NOT NULL,
    PRIMARY KEY(tenant, plane, application, credential_revision)
) STRICT, WITHOUT ROWID;
CREATE TABLE tombstones (
    tenant TEXT NOT NULL,
    kind TEXT NOT NULL,
    object_id TEXT NOT NULL,
    created_ms INTEGER NOT NULL,
    job_id TEXT REFERENCES jobs(job_id),
    PRIMARY KEY(tenant, kind, object_id)
) STRICT, WITHOUT ROWID;
CREATE TABLE schedules (
    tenant TEXT NOT NULL,
    name TEXT NOT NULL,
    enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
    spec_json TEXT NOT NULL CHECK(json_valid(spec_json)),
    effective_ms INTEGER NOT NULL,
    PRIMARY KEY(tenant, name)
) STRICT, WITHOUT ROWID;
CREATE TABLE schedule_slots (
    tenant TEXT NOT NULL,
    name TEXT NOT NULL,
    slot TEXT NOT NULL,
    job_id TEXT REFERENCES jobs(job_id),
    state TEXT NOT NULL CHECK(state IN ('pending','submitted','completed','skipped')),
    PRIMARY KEY(tenant, name, slot)
) STRICT, WITHOUT ROWID;
-- Administrative authority (contract §3). Each transition in authority_store.rs is one
-- transaction over these tables (and `jobs` for the job-creating transitions).
CREATE TABLE previews (
    preview_id TEXT PRIMARY KEY,
    tenant TEXT NOT NULL,
    epoch TEXT NOT NULL REFERENCES epochs(epoch),
    action TEXT NOT NULL,
    sub TEXT NOT NULL,
    client_id TEXT NOT NULL,
    digest TEXT NOT NULL CHECK(length(digest)=64),
    -- The canonical action parameters the digest is computed from (§10: durable inputs).
    params_json TEXT NOT NULL CHECK(json_valid(params_json)),
    generation INTEGER NOT NULL CHECK(generation >= 0),
    nonce TEXT NOT NULL UNIQUE CHECK(length(nonce)=32),
    created_ms INTEGER NOT NULL,
    expires_ms INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'open' CHECK(state IN ('open','approved','consumed')),
    approval_id TEXT,
    upload_id TEXT REFERENCES upload_sessions(upload_id),
    consumed_job_id TEXT REFERENCES jobs(job_id),
    CHECK((action = 'import_finalize') = (upload_id IS NOT NULL)),
    CHECK((state = 'approved') <= (approval_id IS NOT NULL))
) STRICT, WITHOUT ROWID;
CREATE TABLE approvals (
    approval_id TEXT PRIMARY KEY,
    preview_id TEXT NOT NULL UNIQUE REFERENCES previews(preview_id),
    approver_sub TEXT NOT NULL,
    approver_client_id TEXT NOT NULL,
    agent_sub TEXT NOT NULL,
    agent_client_id TEXT NOT NULL,
    digest TEXT NOT NULL CHECK(length(digest)=64),
    generation INTEGER NOT NULL CHECK(generation >= 0),
    created_ms INTEGER NOT NULL,
    expires_ms INTEGER NOT NULL,
    consumed INTEGER NOT NULL CHECK(consumed IN (0,1)),
    CHECK(approver_sub <> agent_sub OR approver_client_id <> agent_client_id)
) STRICT, WITHOUT ROWID;
CREATE TABLE upload_sessions (
    upload_id TEXT PRIMARY KEY,
    tenant TEXT NOT NULL,
    sub TEXT NOT NULL,
    client_id TEXT NOT NULL,
    preview_id TEXT NOT NULL UNIQUE REFERENCES previews(preview_id),
    total INTEGER NOT NULL CHECK(total >= 0),
    received INTEGER NOT NULL CHECK(received >= 0 AND received <= total),
    created_ms INTEGER NOT NULL,
    expires_ms INTEGER NOT NULL,
    -- `imported` and `aborted` keep their staging reserved until its verified removal
    -- (`removed`).
    state TEXT NOT NULL CHECK(state IN ('receiving','finalized','imported','aborted','removed'))
) STRICT, WITHOUT ROWID;
-- The admitted binding of an authority-class idempotency key (contract §3 rule 6). The
-- result is a job ID (`lifecycle_execute`) or an upload ID (`transfer_upload_begin`).
CREATE TABLE authority_keys (
    tenant TEXT NOT NULL,
    sub TEXT NOT NULL,
    client_id TEXT NOT NULL,
    operation TEXT NOT NULL CHECK(operation IN ('lifecycle_execute','transfer_upload_begin')),
    epoch TEXT NOT NULL REFERENCES epochs(epoch),
    key TEXT NOT NULL CHECK(length(key) BETWEEN 16 AND 64),
    preview_id TEXT NOT NULL REFERENCES previews(preview_id),
    digest TEXT NOT NULL CHECK(length(digest)=64),
    generation INTEGER NOT NULL,
    approval_id TEXT,
    result_id TEXT NOT NULL,
    PRIMARY KEY(tenant, sub, client_id, operation, epoch, key)
) STRICT, WITHOUT ROWID;
CREATE TABLE publications (
    report_version TEXT PRIMARY KEY,
    tenant TEXT NOT NULL,
    manifest_sha256 TEXT NOT NULL CHECK(length(manifest_sha256)=64),
    fence_generation INTEGER NOT NULL,
    published_ms INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
CREATE TABLE fences (
    tenant TEXT NOT NULL,
    scope TEXT NOT NULL,
    generation INTEGER NOT NULL CHECK(generation >= 0),
    PRIMARY KEY(tenant, scope)
) STRICT, WITHOUT ROWID;
CREATE TABLE completion_events (
    event_id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES jobs(job_id),
    kind TEXT NOT NULL,
    created_ms INTEGER NOT NULL,
    delivered INTEGER NOT NULL DEFAULT 0 CHECK(delivered IN (0,1))
) STRICT, WITHOUT ROWID;
CREATE TABLE audit (
    sequence INTEGER PRIMARY KEY,
    at_ms INTEGER NOT NULL,
    sub TEXT,
    client_id TEXT,
    grant_label TEXT,
    operation TEXT NOT NULL,
    job_id TEXT,
    scope_digest TEXT,
    policy_revision TEXT NOT NULL,
    outcome TEXT NOT NULL
) STRICT;
-- `receipts_ingest` results by batch key (contract §4.1: idempotent by key). The archive
-- write happens first; a replay after a crash between the two reports duplicates.
CREATE TABLE ingest_results (
    tenant TEXT NOT NULL,
    sub TEXT NOT NULL,
    client_id TEXT NOT NULL,
    epoch TEXT NOT NULL REFERENCES epochs(epoch),
    key TEXT NOT NULL,
    digest TEXT NOT NULL CHECK(length(digest)=64),
    result_json TEXT NOT NULL CHECK(json_valid(result_json)),
    created_ms INTEGER NOT NULL,
    PRIMARY KEY(tenant, sub, client_id, epoch, key)
) STRICT, WITHOUT ROWID;
