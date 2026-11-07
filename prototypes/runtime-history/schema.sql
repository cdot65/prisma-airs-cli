-- Candidate schema 1: immutable normalized observations and rebuildable projections.
-- B0 prototype only until the separately reviewed repository layer adopts it.
PRAGMA application_id = 1095324243; -- AIRS
PRAGMA user_version = 1;
CREATE TABLE archive_meta (
    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
    cloud_id TEXT NOT NULL UNIQUE,
    tsg_id TEXT NOT NULL,
    environment_id TEXT NOT NULL,
    environment_json TEXT NOT NULL CHECK(json_valid(environment_json)),
    created_ms INTEGER NOT NULL,
    analysis_version INTEGER NOT NULL CHECK(analysis_version >= 1),
    redaction_version INTEGER NOT NULL,
    bootstrap_ms INTEGER
) STRICT;
CREATE TABLE collection_runs (
    run_id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK(kind IN ('sync','refresh','scan','bulk','resume','import')),
    started_ms INTEGER NOT NULL,
    ended_ms INTEGER,
    target_since_ms INTEGER,
    target_until_ms INTEGER,
    state TEXT NOT NULL CHECK(state IN ('running','complete','partial','interrupted','failed')),
    provenance_json TEXT NOT NULL CHECK(json_valid(provenance_json))
) STRICT, WITHOUT ROWID;
CREATE TABLE events (
    event_id TEXT PRIMARY KEY,
    app_id TEXT NOT NULL,
    app_name TEXT NOT NULL,
    session_id TEXT NOT NULL,
    scan_id TEXT NOT NULL,
    sub_request INTEGER NOT NULL CHECK(sub_request >= 0),
    report_id TEXT,
    event_ms INTEGER,
    first_seen_ms INTEGER NOT NULL,
    last_seen_ms INTEGER NOT NULL,
    profile TEXT,
    verdict TEXT NOT NULL CHECK(verdict IN ('benign','malicious','unknown','error')),
    action TEXT NOT NULL CHECK(action IN ('allow','block','unknown','failed')),
    conflicting INTEGER NOT NULL CHECK(conflicting IN (0,1)),
    UNIQUE(app_id,app_name,session_id,scan_id,sub_request)
) STRICT, WITHOUT ROWID;
CREATE INDEX events_time_scope ON events(event_ms,app_id,verdict,action,event_id,scan_id);
CREATE INDEX events_app_time ON events(app_id,event_ms,event_id);
CREATE INDEX events_scan_context ON events(scan_id,sub_request,session_id,app_id,app_name);
CREATE TABLE event_facts (
    fact_id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL REFERENCES events(event_id) ON DELETE CASCADE,
    source TEXT NOT NULL,
    observed_ms INTEGER NOT NULL,
    redaction_version INTEGER NOT NULL,
    fields_json TEXT NOT NULL CHECK(json_valid(fields_json))
) STRICT, WITHOUT ROWID;
CREATE INDEX facts_event ON event_facts(event_id,fact_id);
CREATE TABLE fact_runs (
    fact_id TEXT NOT NULL REFERENCES event_facts(fact_id) ON DELETE CASCADE,
    run_id TEXT NOT NULL REFERENCES collection_runs(run_id) ON DELETE CASCADE,
    observed_ms INTEGER NOT NULL,
    PRIMARY KEY(fact_id,run_id)
) STRICT, WITHOUT ROWID;
CREATE TABLE detector_observations (
    event_id TEXT NOT NULL REFERENCES events(event_id) ON DELETE CASCADE,
    detector TEXT NOT NULL,
    direction TEXT NOT NULL CHECK(direction IN ('prompt','response','tool','unknown')),
    verdict TEXT NOT NULL CHECK(verdict IN ('benign','malicious','unknown','error')),
    action TEXT NOT NULL CHECK(action IN ('allow','block','unknown','failed')),
    conflicting INTEGER NOT NULL CHECK(conflicting IN (0,1)),
    provenance_json TEXT NOT NULL CHECK(json_valid(provenance_json)),
    PRIMARY KEY(event_id,detector,direction)
) STRICT, WITHOUT ROWID;
CREATE INDEX detector_filter ON detector_observations(detector,event_id,direction);
CREATE TABLE receipts (
    receipt_id TEXT PRIMARY KEY,
    origin_key TEXT NOT NULL UNIQUE,
    first_seen_ms INTEGER NOT NULL,
    last_seen_ms INTEGER NOT NULL,
    scan_id TEXT,
    app_id TEXT,
    app_name TEXT,
    session_id TEXT,
    sub_request INTEGER CHECK(sub_request >= 0),
    report_id TEXT,
    binding_json TEXT NOT NULL CHECK(json_valid(binding_json)),
    state TEXT NOT NULL CHECK(state IN ('partial','promoted','ambiguous','conflicting')),
    promoted_event_id TEXT REFERENCES events(event_id) ON DELETE RESTRICT,
    CHECK((state = 'promoted') = (promoted_event_id IS NOT NULL))
) STRICT, WITHOUT ROWID;
CREATE INDEX receipt_candidates ON receipts(scan_id,sub_request,session_id,app_id);
CREATE TABLE receipt_facts (
    fact_id TEXT PRIMARY KEY,
    receipt_id TEXT NOT NULL REFERENCES receipts(receipt_id) ON DELETE CASCADE,
    observed_ms INTEGER NOT NULL,
    fields_json TEXT NOT NULL CHECK(json_valid(fields_json))
) STRICT, WITHOUT ROWID;
CREATE TABLE examples (
    example_id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL REFERENCES events(event_id) ON DELETE CASCADE,
    report_id TEXT NOT NULL,
    report_row INTEGER NOT NULL CHECK(report_row >= 0),
    detector TEXT NOT NULL,
    direction TEXT NOT NULL,
    family TEXT NOT NULL CHECK(family IN ('pi','dlp','dbs','tc')),
    state TEXT NOT NULL CHECK(state IN ('absent','empty','present','withheld','unavailable')),
    excerpts_json TEXT NOT NULL CHECK(json_valid(excerpts_json)),
    redaction_version INTEGER NOT NULL,
    observed_ms INTEGER NOT NULL,
    binding_json TEXT NOT NULL CHECK(json_valid(binding_json))
) STRICT, WITHOUT ROWID;
CREATE INDEX examples_event ON examples(event_id,observed_ms);
CREATE TABLE profile_observations (
    observation_id TEXT PRIMARY KEY,
    profile_id TEXT NOT NULL,
    profile_name TEXT NOT NULL,
    revision INTEGER,
    observed_ms INTEGER NOT NULL,
    run_id TEXT NOT NULL REFERENCES collection_runs(run_id),
    fields_json TEXT NOT NULL CHECK(json_valid(fields_json))
) STRICT, WITHOUT ROWID;
CREATE INDEX profiles_asof ON profile_observations(profile_id,observed_ms DESC);
CREATE TABLE aggregate_observations (
    observation_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES collection_runs(run_id),
    observed_ms INTEGER NOT NULL,
    source TEXT NOT NULL,
    population TEXT NOT NULL,
    unit TEXT NOT NULL,
    scale TEXT,
    window_json TEXT NOT NULL CHECK(json_valid(window_json)),
    fields_json TEXT NOT NULL CHECK(json_valid(fields_json))
) STRICT, WITHOUT ROWID;
CREATE INDEX aggregates_source_time ON aggregate_observations(source,observed_ms);
CREATE TABLE coverage_segments (
    segment_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES collection_runs(run_id),
    app_id TEXT,
    app_name TEXT,
    source TEXT NOT NULL,
    since_ms INTEGER NOT NULL,
    until_ms INTEGER NOT NULL CHECK(until_ms > since_ms),
    observed_ms INTEGER NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('observed','traversed_as_of','partial','unavailable','falling_behind')),
    fields_json TEXT NOT NULL CHECK(json_valid(fields_json))
) STRICT, WITHOUT ROWID;
CREATE INDEX coverage_scope_time ON coverage_segments(source,app_id,since_ms,until_ms);
CREATE TABLE sync_work (
    work_id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK(kind IN ('session','results','reports')),
    descriptor_json TEXT NOT NULL CHECK(json_valid(descriptor_json)),
    state TEXT NOT NULL CHECK(state IN ('pending','complete','deferred','ineligible','falling_behind')),
    binding_revision TEXT,
    retry_epoch INTEGER NOT NULL DEFAULT 0 CHECK(retry_epoch >= 0),
    attempts INTEGER NOT NULL CHECK(attempts >= 0 AND attempts <= 3),
    last_attempt_ms INTEGER,
    eligible_after_ms INTEGER,
    reason TEXT,
    provenance_json TEXT NOT NULL CHECK(json_valid(provenance_json)),
    UNIQUE(work_id,kind),
    CHECK(kind != 'session' OR (attempts = 0 AND retry_epoch = 0 AND binding_revision IS NULL))
) STRICT, WITHOUT ROWID;
CREATE INDEX work_eligible ON sync_work(kind,state,eligible_after_ms,work_id);
-- Counter resets and credential revisions never erase prior transport evidence.
CREATE TABLE work_attempts (
    attempt_id TEXT PRIMARY KEY,
    work_id TEXT NOT NULL,
    source TEXT NOT NULL CHECK(source IN ('results','reports')),
    run_id TEXT NOT NULL REFERENCES collection_runs(run_id) ON DELETE RESTRICT,
    transport_id TEXT NOT NULL,
    binding_revision TEXT NOT NULL,
    retry_epoch INTEGER NOT NULL CHECK(retry_epoch >= 0),
    ordinal INTEGER NOT NULL CHECK(ordinal >= 1 AND ordinal <= 3),
    started_ms INTEGER NOT NULL,
    ended_ms INTEGER,
    status INTEGER CHECK(status >= 100 AND status <= 599),
    outcome TEXT NOT NULL CHECK(outcome IN ('started','not_sent','returned','missing','pending','rejected','auth_denied','rate_limited','failed','interrupted')),
    provenance_json TEXT NOT NULL CHECK(json_valid(provenance_json)),
    FOREIGN KEY(work_id,source) REFERENCES sync_work(work_id,kind) ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;
CREATE UNIQUE INDEX attempt_ordinals ON work_attempts(work_id,binding_revision,retry_epoch,ordinal)
    WHERE outcome != 'not_sent';
CREATE INDEX attempts_work ON work_attempts(work_id,started_ms);
CREATE TABLE work_transitions (
    transition_id TEXT PRIMARY KEY,
    work_id TEXT NOT NULL REFERENCES sync_work(work_id) ON DELETE RESTRICT,
    occurred_ms INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('retry_reset','binding_changed','deferred','ineligible','cooldown','completed','falling_behind')),
    retry_epoch INTEGER NOT NULL CHECK(retry_epoch >= 0),
    old_binding_revision TEXT,
    new_binding_revision TEXT,
    reason TEXT NOT NULL,
    provenance_json TEXT NOT NULL CHECK(json_valid(provenance_json))
) STRICT, WITHOUT ROWID;
CREATE INDEX transitions_work ON work_transitions(work_id,occurred_ms);
-- Management traversal is bounded by invocation budgets, not scanner retry epochs.
CREATE TABLE management_reads (
    read_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES collection_runs(run_id) ON DELETE RESTRICT,
    source TEXT NOT NULL,
    app_id TEXT,
    app_name TEXT,
    started_ms INTEGER NOT NULL,
    ended_ms INTEGER,
    status INTEGER CHECK(status >= 100 AND status <= 599),
    actual_sends INTEGER NOT NULL CHECK(actual_sends IN (0,1)),
    returned_rows INTEGER CHECK(returned_rows >= 0),
    outcome TEXT NOT NULL CHECK(outcome IN ('started','not_sent','returned','auth_denied','rate_limited','failed','interrupted')),
    descriptor_json TEXT NOT NULL CHECK(json_valid(descriptor_json))
) STRICT, WITHOUT ROWID;
CREATE INDEX management_reads_run ON management_reads(run_id,source,started_ms);
CREATE TABLE service_cooldowns (
    service TEXT PRIMARY KEY,
    until_ms INTEGER NOT NULL,
    observed_ms INTEGER NOT NULL,
    reason TEXT NOT NULL
) STRICT, WITHOUT ROWID;
