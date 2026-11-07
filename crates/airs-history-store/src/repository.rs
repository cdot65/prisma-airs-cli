//! Normalized immutable facts and rebuildable projections; no SDK model serialization.
use super::{
    database::{self, Reader, Writer, sql_error},
    failure,
    identity::Cloud,
};
use airs_runtime_core::{
    error::{CliError, SecretSet},
    signal::{
        detectors,
        evidence::{Application, Event, Evidence, Metric, Profile, Source, Status},
        privacy::{self, PayloadSpans},
    },
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::Duration,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventFact {
    event: Event,
    #[serde(default, skip_serializing_if = "is_false")]
    identity_withheld: bool,
    origin: String,
    scanner_endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scanner_reference: Option<airs_runtime_core::tenant::scanners::ScannerReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    batch_report_id: Option<String>,
}
fn is_false(value: &bool) -> bool {
    !value
}

#[derive(Default)]
struct DetectorProjection {
    verdicts: BTreeSet<String>,
    actions: BTreeSet<String>,
    positive_metadata: bool,
    positive_hint_event_sources: BTreeSet<String>,
    report_observed: bool,
    report_event_sources: BTreeSet<String>,
    report_ids: BTreeSet<String>,
    origins: BTreeSet<String>,
    comparison_subsumed: bool,
    subsumed_hint_event_sources: BTreeSet<String>,
}

enum Record {
    Event(EventFact),
    Receipt {
        origin: String,
        fact: EventFact,
    },
    Profile {
        profile: Profile,
        key: String,
        linked: bool,
    },
    Application {
        application: Application,
        key: String,
        linked: bool,
    },
    Metric {
        metric: Metric,
        window: String,
        population_key: String,
        linked: bool,
    },
    Coverage {
        source: Source,
        since: i64,
        until: i64,
        selected_apps: Vec<String>,
        scope_supported: bool,
        native: bool,
    },
}
impl Record {
    fn bytes(&self) -> Result<usize, CliError> {
        Ok(match self {
            Self::Event(f) | Self::Receipt { fact: f, .. } => encoded(f)?.len(),
            Self::Profile { profile, key, .. } => encoded(profile)?.len() + key.len(),
            Self::Application {
                application, key, ..
            } => encoded(application)?.len() + key.len(),
            Self::Metric {
                metric,
                window,
                population_key,
                ..
            } => encoded(metric)?.len() + window.len() + population_key.len(),
            Self::Coverage {
                source,
                selected_apps,
                ..
            } => encoded(source)?.len() + encoded(selected_apps)?.len(),
        } + 1024)
    }
}

fn integer(value: u64) -> Result<i64, CliError> {
    i64::try_from(value).map_err(|_| failure("integer exceeds the supported archive range"))
}

/// One bounded immutable input snapshot. A FIFO/symlink cannot block admission;
/// parsing, digest/version checks and privacy projection precede archive creation.
pub fn read_legacy(path: &Path) -> Result<Evidence, CliError> {
    use std::{fs, io::Read, os::unix::fs::MetadataExt};
    let before =
        fs::symlink_metadata(path).map_err(|_| failure("cannot inspect legacy evidence input"))?;
    if !before.is_file() || before.nlink() != 1 || before.len() > 32 * 1024 * 1024 {
        return Err(failure(
            "legacy input must be a regular single-link file of at most 32 MiB",
        ));
    }
    let mut file: fs::File = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| failure("cannot open legacy evidence input"))?
    .into();
    let admitted = file
        .metadata()
        .map_err(|_| failure("cannot inspect legacy evidence descriptor"))?;
    if !admitted.is_file()
        || admitted.nlink() != 1
        || (before.dev(), before.ino()) != (admitted.dev(), admitted.ino())
    {
        return Err(failure("legacy input identity changed"));
    }
    let mut bytes = Vec::new();
    Read::take(&mut file, 32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| failure("cannot read legacy evidence input"))?;
    let after = file
        .metadata()
        .map_err(|_| failure("cannot recheck legacy evidence input"))?;
    if bytes.len() > 32 * 1024 * 1024
        || (
            admitted.len(),
            admitted.mtime(),
            admitted.mtime_nsec(),
            admitted.ctime(),
            admitted.ctime_nsec(),
        ) != (
            after.len(),
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec(),
        )
    {
        return Err(failure(
            "legacy input changed while being read or exceeded its bound",
        ));
    }
    let text =
        std::str::from_utf8(&bytes).map_err(|_| failure("legacy input is not valid UTF-8"))?;
    Evidence::parse(text)
}

/// Validate the normalized fact schema at the logical-transfer boundary. This
/// never reconstructs a join from a value removed by privacy projection.
/// Used only by the unfinished B4 transfer module (see `history/mod.rs`).
#[allow(dead_code)]
pub(super) fn validate_transfer_fact(
    value: &serde_json::Value,
    full: bool,
) -> Result<(), CliError> {
    let fact: EventFact = serde_json::from_value(value.clone())
        .map_err(|_| failure("logical transfer fact has unknown fields or invalid types"))?;
    let e = &fact.event;
    let mut detection_rows = BTreeSet::new();
    let invalid_ownership = e.report_ids.iter().collect::<BTreeSet<_>>().len()
        != e.report_ids.len()
        || e.detections.iter().any(|d| {
            !e.report_ids.contains(&d.report_id)
                || !detection_rows.insert((d.report_id.as_str(), d.row))
                || d.snippets
                    .iter()
                    .map(|s| s.family.as_str())
                    .collect::<BTreeSet<_>>()
                    .len()
                    != d.snippets.len()
        });
    if invalid_ownership
        || !["benign", "malicious", "unknown", "error"].contains(&e.verdict.as_str())
        || !["allow", "block", "unknown", "failed"].contains(&e.action.as_str())
        || (!e.identity.scan_id.is_empty()
            && uuid::Uuid::parse_str(&e.identity.scan_id)
                .map(|id| id.to_string())
                .ok()
                .as_deref()
                != Some(e.identity.scan_id.as_str()))
        || e.identity
            .sub_request
            .is_some_and(|n| i64::try_from(n).is_err())
        || e.detections.iter().any(|d| {
            !["prompt", "response", "tool", "unknown"].contains(&d.direction.as_str())
                || !["benign", "malicious", "unknown", "error"].contains(&d.verdict.as_str())
                || !["allow", "block", "unknown", "failed"].contains(&d.action.as_str())
                || i64::try_from(d.row).is_err()
                || d.snippets.iter().any(|snippet| {
                    !["pi", "dlp", "dbs", "tc"].contains(&snippet.family.as_str())
                        || snippet.excerpts.len() > 20
                        || (snippet.state
                            == airs_runtime_core::signal::evidence::SnippetState::Present
                            && snippet.excerpts.is_empty())
                        || (snippet.state
                            != airs_runtime_core::signal::evidence::SnippetState::Present
                            && !snippet.excerpts.is_empty())
                })
        })
        || (full
            && ([
                &e.identity.app_id,
                &e.identity.app_name,
                &e.identity.session_id,
                &e.identity.scan_id,
            ]
            .iter()
            .any(|s| withheld(s))
                || e.identity.app_id.trim().is_empty()
                || e.identity.app_name.trim().is_empty()
                || e.identity.session_id.trim().is_empty()
                || e.identity.scan_id.is_empty()
                || e.identity.sub_request.is_none()))
        || fact.scanner_reference.as_ref().is_some_and(|r| {
            !r.valid()
                || r.application_id != e.identity.app_id
                || Some(r.endpoint.as_str()) != fact.scanner_endpoint.as_deref()
        })
    {
        return Err(failure(
            "logical transfer fact has unsupported normalized identity or outcomes",
        ));
    }
    Ok(())
}

fn encoded<T: Serialize>(value: &T) -> Result<String, CliError> {
    serde_json::to_string(value).map_err(|_| failure("cannot encode normalized archive data"))
}
fn digest<T: Serialize>(value: &T) -> Result<String, CliError> {
    Ok(format!("{:x}", Sha256::digest(encoded(value)?.as_bytes())))
}
fn consensus(values: &BTreeSet<String>, unknown: &str) -> String {
    if values.len() == 1 {
        values.first().cloned().unwrap_or_else(|| unknown.into())
    } else {
        unknown.into()
    }
}
fn known(set: &mut BTreeSet<String>, value: &str) {
    if value != "unknown" && !value.is_empty() {
        set.insert(value.to_owned());
    }
}
fn optional(set: &BTreeSet<String>) -> Option<String> {
    (set.len() == 1).then(|| set.first().cloned()).flatten()
}
fn withheld(value: &str) -> bool {
    [
        privacy::WITHHELD,
        airs_runtime_core::error::WITHHELD,
        "[REDACTED]",
    ]
    .contains(&value)
}

#[derive(Serialize)]
pub struct ImportSummary {
    pub collection_id: String,
    pub processed_records: usize,
    pub batches: usize,
    pub input_digest: String,
    pub privacy_projected: bool,
    pub input_partial_receipts: usize,
    pub input_unlinked_profiles: usize,
    pub input_unlinked_metrics: usize,
}

/// Every content check of [`import_evidence`] (normalization, privacy projection and
/// bounds of every record) without any archive: the record count of a valid legacy
/// bundle, or the refusal. Used when a staged `legacy_bundle` upload is finalized.
pub fn validate_evidence(
    cloud: &Cloud,
    original: &Evidence,
    secrets: &SecretSet,
) -> Result<u64, CliError> {
    cloud.check_public(secrets)?;
    let prepared = prepare_evidence(original, secrets, &PayloadSpans::default(), false)?;
    Ok(prepared.records.len() as u64)
}

/// Legacy import is explicitly operator-bound by the adapter. Validate/normalize
/// every record before creating any archive. Each committed batch is idempotent.
pub fn import_evidence(
    root: &Path,
    cloud: &Cloud,
    original: &Evidence,
    secrets: &SecretSet,
    payload: &PayloadSpans,
    session: &scm::Session,
) -> Result<ImportSummary, CliError> {
    cloud.check_public(secrets)?;
    let Prepared {
        safe,
        records,
        sizes,
        privacy_projected,
    } = prepare_evidence(original, secrets, payload, false)?;
    let provenance = encoded(
        &serde_json::json!({"origin":"legacy_import/operator_bound","original_collection_id":original.collection_id,"original_digest":original.digest,"projected_digest":safe.digest,"legacy_tenant":safe.tenant,"window":safe.window,"selected_app_ids":safe.app_ids,"evidence_schema_version":original.schema_version,"rules_version":original.rules_version,"redaction_version":original.redaction_version,"sdk_version":original.sdk_version}),
    )?;
    let started = database::now_ms()?;
    let input_partial_receipts = records
        .iter()
        .filter(|r| matches!(r, Record::Receipt { .. }))
        .count();
    let input_unlinked_profiles = records
        .iter()
        .filter(|r| matches!(r, Record::Profile { linked: false, .. }))
        .count();
    let input_unlinked_metrics = records
        .iter()
        .filter(|r| matches!(r, Record::Metric { linked: false, .. }))
        .count();
    let until = std::time::Instant::now() + Duration::from_secs(120);
    let lease = {
        let w = Writer::open(root, cloud, Duration::from_secs(120), true, session)?;
        let prior: Option<(String, Option<i64>, String)> = w
            .conn
            .query_row(
                "SELECT provenance_json,ended_ms,state FROM collection_runs WHERE run_id=?1",
                [&safe.collection_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(sql_error)?;
        if prior.as_ref().is_some_and(|(p, _, _)| p != &provenance) {
            return Err(failure(
                "collection ID already has different immutable provenance; archive was not changed",
            ));
        }
        if prior.as_ref().is_some_and(|(_, ended, state)| {
            ended.is_some() && ["complete", "partial"].contains(&state.as_str())
        }) {
            return Ok(ImportSummary {
                collection_id: safe.collection_id,
                processed_records: 0,
                batches: 0,
                input_digest: original.digest.clone(),
                privacy_projected,
                input_partial_receipts,
                input_unlinked_profiles,
                input_unlinked_metrics,
            });
        }
        w.check()?;
        w.conn.execute("INSERT OR IGNORE INTO collection_runs VALUES (?1,'import',?2,NULL,?3,?4,'running',?5)",params![safe.collection_id,started,safe.window.start_ms,safe.window.end_ms,provenance]).map_err(sql_error)?;
        w.into_lease()?
    };
    let mut batches = 0;
    let mut next = 0;
    while next < records.len() {
        let mut end = next;
        let mut bytes = 0;
        while end < records.len() && end - next < 200 && bytes + sizes[end] <= 256 * 1024 {
            bytes += sizes[end];
            end += 1;
        }
        let remaining = until.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(failure(
                "import deadline exceeded; committed batches remain labeled incomplete",
            ));
        }
        lease.check()?;
        let mut w = Writer::open(root, cloud, remaining, false, session)?;
        w.check()?;
        let tx = w.conn.transaction().map_err(sql_error)?;
        for record in &records[next..end] {
            if session.is_cancelled() || std::time::Instant::now() >= until {
                return Err(failure(
                    "import interrupted or timed out; current batch was rolled back",
                ));
            }
            store(&tx, record, &safe.collection_id, safe.collected_ms)?;
        }
        if session.is_cancelled() || std::time::Instant::now() >= until {
            return Err(failure(
                "import interrupted or timed out; current batch was rolled back",
            ));
        }
        tx.commit().map_err(sql_error)?;
        w.check()?;
        drop(w);
        next = end;
        batches += 1;
        pause_after_batch(session);
    }
    let remaining = until.saturating_duration_since(std::time::Instant::now());
    if remaining.is_zero() {
        return Err(failure(
            "import deadline exceeded before final provenance commit",
        ));
    }
    lease.check()?;
    let w = Writer::open(root, cloud, remaining, false, session)?;
    w.check()?;
    w.conn
        .execute(
            "UPDATE collection_runs SET ended_ms=?2,state=?3 WHERE run_id=?1",
            params![
                safe.collection_id,
                database::now_ms()?,
                if safe.incomplete() > 0 {
                    "partial"
                } else {
                    "complete"
                }
            ],
        )
        .map_err(sql_error)?;
    Ok(ImportSummary {
        collection_id: safe.collection_id,
        processed_records: records.len(),
        batches,
        input_digest: original.digest.clone(),
        privacy_projected,
        input_partial_receipts,
        input_unlinked_profiles,
        input_unlinked_metrics,
    })
}

struct Prepared {
    safe: Evidence,
    records: Vec<Record>,
    sizes: Vec<usize>,
    privacy_projected: bool,
}
fn prepare_evidence(
    original: &Evidence,
    secrets: &SecretSet,
    payload: &PayloadSpans,
    native: bool,
) -> Result<Prepared, CliError> {
    if native {
        original.validate_native()?;
    } else {
        original.validate()?;
    }
    let mut safe = original.clone();
    privacy::project_with_spans(&mut safe, secrets, payload)?;
    let privacy_projected = original.digest != safe.digest()?;
    if native {
        safe.seal_native()?;
    } else {
        safe.seal()?;
    }
    let mut records = Vec::new();
    for (index, event) in safe.events.iter().enumerate() {
        let mut event = event.clone();
        if !["benign", "malicious", "unknown", "error"].contains(&event.verdict.as_str())
            || !["allow", "block", "unknown", "failed"].contains(&event.action.as_str())
            || event.detections.iter().any(|d| {
                !["prompt", "response", "tool", "unknown"].contains(&d.direction.as_str())
                    || !["benign", "malicious", "unknown", "error"].contains(&d.verdict.as_str())
                    || !["allow", "block", "unknown", "failed"].contains(&d.action.as_str())
                    || i64::try_from(d.row).is_err()
            })
        {
            return Err(failure(
                "legacy evidence has unsupported outcome/direction/row values; input was not imported",
            ));
        }
        if event
            .identity
            .sub_request
            .is_some_and(|v| i64::try_from(v).is_err())
        {
            return Err(failure(
                "subrequest exceeds the supported archive range; input was not imported",
            ));
        }
        event.report_ids.sort();
        event.report_ids.dedup();
        event.detector_hints.sort();
        event.detector_hints.dedup();
        event.sources.sort();
        event.sources.dedup();
        event.notes.sort();
        event.notes.dedup();
        event
            .detections
            .sort_by_cached_key(|d| serde_json::to_string(d).unwrap_or_default());
        event.detections.dedup();
        let original_identity = &original
            .events
            .get(index)
            .ok_or_else(|| failure("legacy event ordering changed"))?
            .identity;
        let identity_changed = original_identity != &event.identity
            || [
                &event.identity.app_id,
                &event.identity.app_name,
                &event.identity.session_id,
                &event.identity.scan_id,
            ]
            .iter()
            .any(|s| withheld(s));
        if identity_changed {
            if original_identity.app_id != event.identity.app_id || withheld(&event.identity.app_id)
            {
                event.identity.app_id.clear();
            }
            if original_identity.app_name != event.identity.app_name
                || withheld(&event.identity.app_name)
            {
                event.identity.app_name.clear();
            }
            if original_identity.session_id != event.identity.session_id
                || withheld(&event.identity.session_id)
            {
                event.identity.session_id.clear();
            }
            if original_identity.scan_id != event.identity.scan_id
                || withheld(&event.identity.scan_id)
            {
                event.identity.scan_id.clear();
            }
        }
        if event.profile.as_ref().is_some_and(|p| withheld(p))
            || original.events[index].profile != event.profile
        {
            event.profile = None;
            event
                .notes
                .push("Profile association is unavailable after privacy projection".into());
        }
        if let Ok(uuid) = uuid::Uuid::parse_str(&event.identity.scan_id) {
            event.identity.scan_id = uuid.to_string();
        }
        let full = !identity_changed
            && !event.identity.app_id.trim().is_empty()
            && !event.identity.app_name.trim().is_empty()
            && !event.identity.session_id.trim().is_empty()
            && uuid::Uuid::parse_str(&event.identity.scan_id).is_ok()
            && event
                .identity
                .sub_request
                .is_some_and(|n| i64::try_from(n).is_ok());
        let identity_withheld = identity_changed
            || original.events[index]
                .report_ids
                .iter()
                .any(|id| withheld(id) || !event.report_ids.contains(id));
        let fact = EventFact {
            event,
            identity_withheld,
            origin: "legacy_import/operator_bound".into(),
            scanner_endpoint: None,
            scanner_reference: None,
            batch_report_id: None,
        };
        records.push(if full {
            Record::Event(fact)
        } else {
            Record::Receipt {
                origin: format!("legacy/{}/event/{index}", safe.collection_id),
                fact,
            }
        });
    }
    for (index, profile) in safe.profiles.iter().enumerate() {
        if profile.revision.is_some_and(|v| i64::try_from(v).is_err()) {
            return Err(failure(
                "profile revision exceeds the supported archive range",
            ));
        }
        let mut profile = profile.clone();
        profile
            .protections
            .sort_by_cached_key(|p| serde_json::to_string(p).unwrap_or_default());
        profile.protections.dedup();
        let linked = !withheld(&profile.name) && original.profiles[index].name == profile.name;
        let key = if linked {
            format!("name:{}", digest(&profile.name)?)
        } else {
            format!("unlinked:{}/profile/{index}", safe.collection_id)
        };
        records.push(Record::Profile {
            profile,
            key,
            linked,
        });
    }
    for (index, application) in safe.applications.iter().enumerate() {
        let original = &original.applications[index];
        let mut application = application.clone();
        let linked = original.id == application.id
            && original.name == application.name
            && !application.id.trim().is_empty()
            && !application.name.trim().is_empty()
            && !withheld(&application.id)
            && !withheld(&application.name);
        if original.profile != application.profile
            || application.profile.as_ref().is_some_and(|p| withheld(p))
        {
            application.profile = None;
        }
        let key = if linked {
            format!(
                "application:{}",
                digest(&(&application.id, &application.name))?
            )
        } else {
            format!("unlinked:{}/application/{index}", safe.collection_id)
        };
        records.push(Record::Application {
            application,
            key,
            linked,
        });
    }
    let window = encoded(
        &serde_json::json!({"requested":safe.window,"semantics":"uncertain rolling source window"}),
    )?;
    let mut private_populations: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, metric) in safe.metrics.iter().enumerate() {
        let linked = !withheld(&metric.population)
            && original.metrics[index].population == metric.population;
        let population_key = if linked {
            format!("label:{}", digest(&metric.population)?)
        } else {
            // Preserve proven equality within this one collection without storing
            // the private label, a fragment, or a hash derived from that label.
            if withheld(&original.metrics[index].population) {
                format!("unlinked:{}/population/unknown/{index}", safe.collection_id)
            } else {
                let next = private_populations.len();
                let ordinal = *private_populations
                    .entry(&original.metrics[index].population)
                    .or_insert(next);
                format!(
                    "unlinked:{}/population/observed-equality/{ordinal}",
                    safe.collection_id
                )
            }
        };
        records.push(Record::Metric {
            metric: metric.clone(),
            window: window.clone(),
            population_key,
            linked,
        });
    }
    for source in &safe.sources {
        records.push(Record::Coverage {
            source: source.clone(),
            since: safe.window.start_ms,
            until: safe.window.end_ms,
            native: false,
            selected_apps: safe.app_ids.clone(),
            scope_supported: !safe.app_ids.is_empty()
                && safe.app_ids == original.app_ids
                && safe.app_ids.iter().all(|id| !withheld(id)),
        });
    }
    let mut sizes = Vec::with_capacity(records.len());
    for record in &records {
        let bytes = record.bytes()?;
        if bytes > 256 * 1024 {
            return Err(failure(
                "a normalized record exceeds the 256 KiB batch bound; input was not imported",
            ));
        }
        sizes.push(bytes);
    }
    Ok(Prepared {
        safe,
        records,
        sizes,
        privacy_projected,
    })
}

/// A privacy-projected native snapshot, normalized before taking a database gate.
pub(super) struct NativeRecords {
    require_existing: bool,
    records: Vec<Record>,
    sizes: Vec<usize>,
}
impl NativeRecords {
    pub fn identities(&self) -> BTreeSet<airs_runtime_core::signal::evidence::Identity> {
        self.records
            .iter()
            .filter_map(|r| match r {
                Record::Event(f) => Some(f.event.identity.clone()),
                _ => None,
            })
            .collect()
    }
    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn batch_end(&self, start: usize) -> usize {
        let mut end = start;
        let mut bytes = 0;
        while end < self.records.len() && end - start < 200 && bytes + self.sizes[end] <= 256 * 1024
        {
            bytes += self.sizes[end];
            end += 1;
        }
        end
    }
    pub fn store(
        &self,
        conn: &Connection,
        start: usize,
        end: usize,
        run: &str,
        observed: i64,
    ) -> Result<(), CliError> {
        for record in &self.records[start..end] {
            if self.require_existing
                && let Record::Event(fact) = record
            {
                let exists: bool = conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM events WHERE event_id=?1)",
                        [digest(&fact.event.identity)?],
                        |r| r.get(0),
                    )
                    .map_err(sql_error)?;
                if !exists {
                    return Err(failure(
                        "retained work identity is no longer available; revisit was not inserted",
                    ));
                }
            }
            store(conn, record, run, observed)?;
        }
        Ok(())
    }
}

pub(super) fn native_records(
    original: &Evidence,
    secrets: &SecretSet,
    payload: &PayloadSpans,
    references: &BTreeMap<
        airs_runtime_core::signal::evidence::Identity,
        airs_runtime_core::tenant::scanners::ScannerReference,
    >,
    target: (i64, i64),
    origins: &BTreeMap<airs_runtime_core::signal::evidence::Identity, String>,
) -> Result<NativeRecords, CliError> {
    let Prepared { mut records, .. } = prepare_evidence(original, secrets, payload, true)?;
    for (index, record) in records.iter_mut().enumerate() {
        if let Record::Receipt { origin, .. } = record {
            let identity = &original
                .events
                .get(index)
                .ok_or_else(|| failure("native receipt ordering changed"))?
                .identity;
            *origin = origins
                .get(identity)
                .cloned()
                .ok_or_else(|| failure("native receipt origin unavailable"))?;
        }
        match record {
            Record::Event(fact) | Record::Receipt { fact, .. } => {
                fact.origin = "native_sync/projected_observation".into();
                if fact.event.sources.iter().any(|s| s == "scan-results")
                    && let Some(reference) = references.get(&fact.event.identity)
                    && reference.valid()
                    && !secrets.carried_by(&reference.endpoint)
                {
                    fact.scanner_endpoint = Some(reference.endpoint.clone());
                    fact.scanner_reference = Some(reference.clone());
                }
            }
            Record::Metric { window, .. } => {
                *window = encoded(
                    &serde_json::json!({"requested_since_ms":target.0,"requested_until_ms":target.1,"actual_rolling_query":original.window,"semantics":"rolling API observations; absolute bounds applied locally; aggregates are not historical event counts"}),
                )?;
            }
            Record::Coverage {
                native,
                since,
                until,
                scope_supported,
                ..
            } => {
                *scope_supported = original.app_ids.is_empty() || *scope_supported;
                *native = true;
                *since = target.0;
                *until = target.1;
            }
            _ => {}
        }
    }
    // Known timestamps outside the requested interval are not imported by backfill.
    // Unknown timestamps remain explicit observations, never eligible window counts.
    records.retain(|record| match record {
        Record::Event(fact) | Record::Receipt { fact, .. } => fact
            .event
            .timestamp_ms
            .is_none_or(|t| t >= target.0 && t < target.1),
        _ => true,
    });
    let sizes = records
        .iter()
        .map(Record::bytes)
        .collect::<Result<Vec<_>, _>>()?;
    if sizes.iter().any(|n| *n > 256 * 1024) {
        return Err(failure("native normalized record exceeds batch bound"));
    }
    Ok(NativeRecords {
        require_existing: false,
        records,
        sizes,
    })
}

pub(super) fn backlog_records(
    original: &Evidence,
    secrets: &SecretSet,
    payload: &PayloadSpans,
    references: &BTreeMap<
        airs_runtime_core::signal::evidence::Identity,
        airs_runtime_core::tenant::scanners::ScannerReference,
    >,
    origins: &BTreeMap<airs_runtime_core::signal::evidence::Identity, String>,
) -> Result<NativeRecords, CliError> {
    let mut records = native_records(
        original,
        secrets,
        payload,
        references,
        (i64::MIN + 1, i64::MAX),
        origins,
    )?;
    records.records.retain(|r| matches!(r, Record::Event(_)));
    for record in &mut records.records {
        if let Record::Event(fact) = record {
            fact.origin = "native_sync/individual_work_revisit".into();
        }
    }
    records.sizes = records
        .records
        .iter()
        .map(Record::bytes)
        .collect::<Result<Vec<_>, _>>()?;
    records.require_existing = true;
    Ok(records)
}

/// Rehydrate a bounded known full event without inventing an observation time.
/// Immutable facts remain authoritative; current projection supplies consensus.
pub(super) fn cached_event(
    conn: &Connection,
    identity: &airs_runtime_core::signal::evidence::Identity,
) -> Result<Option<Event>, CliError> {
    cached_event_until(conn, identity, None, &ReceiptsAsOf::current())
}

/// [`cached_event`] as of a report's evidence cutoff, for an event with facts first
/// observed after it: its `projection` as of the cutoff and only the facts first observed
/// by then (M6 review 1 finding 1).
pub(super) fn cached_event_as_of(
    conn: &Connection,
    identity: &airs_runtime_core::signal::evidence::Identity,
    cutoff_ms: i64,
    projection: &Projection,
    receipts: &ReceiptsAsOf,
) -> Result<Option<Event>, CliError> {
    cached_event_until(conn, identity, Some((cutoff_ms, projection)), receipts)
}

fn cached_event_until(
    conn: &Connection,
    identity: &airs_runtime_core::signal::evidence::Identity,
    as_of: Option<(i64, &Projection)>,
    receipts: &ReceiptsAsOf,
) -> Result<Option<Event>, CliError> {
    let id = digest(identity)?;
    let cutoff = as_of.map(|(c, _)| c);
    // The rebuild bound counts exactly the facts the rebuild reads: those first observed
    // by the cutoff, with receipt ownership as of it (M6 review 2 finding 2).
    let input_bytes:i64=conn.query_row(&format!("SELECT coalesce((SELECT sum(length(CAST(fields_json AS BLOB))) FROM event_facts WHERE event_id=?1 AND (?2 IS NULL OR observed_ms<=?2)),0)+coalesce((SELECT sum(length(CAST(fields_json AS BLOB))) FROM ({RECEIPT_FACTS_AS_OF})),0)"),params![&id,cutoff,receipts.changed_json(),receipts.owned_json(&id)],|r|r.get(0)).map_err(sql_error)?;
    if input_bytes > 256 * 1024 {
        return Ok(None);
    }
    let row: Option<(Option<i64>, Option<String>, String, String)> = conn
        .query_row(
            "SELECT event_ms,profile,verdict,action FROM events WHERE event_id=?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(sql_error)?;
    let Some((mut at, mut profile, mut verdict, mut action)) = row else {
        return Ok(None);
    };
    if let Some((_, p)) = as_of {
        (at, profile, verdict, action) = (
            p.event_ms,
            p.profile.clone(),
            p.verdict.clone(),
            p.action.clone(),
        );
    }
    let mut unbounded = usize::MAX;
    let mut all = facts_until(
        conn,
        "SELECT fields_json FROM event_facts WHERE event_id=?1 AND (?2 IS NULL OR observed_ms<=?2)",
        &id,
        cutoff,
        &mut unbounded,
    )?;
    all.extend(receipt_facts_until(
        conn,
        &id,
        cutoff,
        receipts,
        &mut unbounded,
    )?);
    let Some(first) = all.first() else {
        return Ok(None);
    };
    let mut event = first.event.clone();
    event.identity = identity.clone();
    event.timestamp_ms = at;
    event.profile = profile;
    event.verdict = verdict;
    event.action = action;
    for fact in all.iter().skip(1) {
        event.report_ids.extend(fact.event.report_ids.clone());
        event.detections.extend(fact.event.detections.clone());
        event
            .detector_hints
            .extend(fact.event.detector_hints.clone());
        event.sources.extend(fact.event.sources.clone());
        event.notes.extend(fact.event.notes.clone());
    }
    event.report_ids.sort();
    event.report_ids.dedup();
    event.detector_hints.sort();
    event.detector_hints.dedup();
    event.sources.sort();
    event.sources.dedup();
    event.notes.sort();
    event.notes.dedup();
    event
        .detections
        .sort_by_cached_key(|d| serde_json::to_string(d).unwrap_or_default());
    event.detections.dedup();
    let mut rows: BTreeMap<(String, u64), Vec<airs_runtime_core::signal::evidence::Detection>> =
        BTreeMap::new();
    for detection in std::mem::take(&mut event.detections) {
        rows.entry((detection.report_id.clone(), detection.row))
            .or_default()
            .push(detection);
    }
    for variants in rows.into_values() {
        let mut shown = variants[0].clone();
        if variants.len() > 1 {
            event.notes.push("One deterministic retained detector-row variant shown; archive preserves all variants".into());
            if variants
                .iter()
                .any(|d| d.detector != shown.detector || d.direction != shown.direction)
            {
                shown.detector = "unknown".into();
                shown.direction = "unknown".into();
                shown.verdict = "unknown".into();
                shown.action = "unknown".into();
                shown.snippets.clear();
            } else {
                if variants.iter().any(|d| d.verdict != shown.verdict) {
                    shown.verdict = "unknown".into();
                }
                if variants.iter().any(|d| d.action != shown.action) {
                    shown.action = "unknown".into();
                }
            }
        }
        event.detections.push(shown);
    }
    if encoded(&event)?.len() > 256 * 1024 {
        return Ok(None);
    }
    Ok(Some(event))
}

pub(super) fn cached_example(
    conn: &Connection,
    identity: &airs_runtime_core::signal::evidence::Identity,
    report: &str,
) -> Result<bool, CliError> {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM examples WHERE event_id=?1 AND report_id=?2 AND state IN ('present','empty','withheld'))",params![digest(identity)?,report],|r|r.get(0)).map_err(sql_error)
}

pub(super) fn retained_contexts(
    conn: &Connection,
    descriptors: &[super::work::Descriptor],
) -> Result<Vec<Event>, CliError> {
    let mut events = vec![];
    let mut bytes = 0;
    let mut seen = BTreeSet::new();
    for descriptor in descriptors {
        let id = &descriptor.identity;
        let mut statement=conn.prepare("SELECT app_id,app_name,session_id,scan_id,sub_request FROM events WHERE app_id=?1 AND scan_id=?2 AND sub_request=?3 ORDER BY app_name,session_id LIMIT 1001").map_err(sql_error)?;
        let rows = statement
            .query_map(
                params![
                    id.app_id,
                    id.scan_id,
                    id.sub_request.map(integer).transpose()?
                ],
                |r| {
                    Ok(airs_runtime_core::signal::evidence::Identity {
                        app_id: r.get(0)?,
                        app_name: r.get(1)?,
                        session_id: r.get(2)?,
                        scan_id: r.get(3)?,
                        sub_request: r
                            .get::<_, Option<i64>>(4)?
                            .and_then(|n| u64::try_from(n).ok()),
                    })
                },
            )
            .map_err(sql_error)?;
        for row in rows {
            let identity = row.map_err(sql_error)?;
            if !seen.insert(identity.clone()) {
                continue;
            }
            bytes += encoded(&identity)?.len();
            if events.len() >= 1000 || bytes > 256 * 1024 {
                return Err(failure(
                    "retained attribution contexts exceed the bounded lookup view; no scanner request sent",
                ));
            }
            events.push(Event {
                identity,
                timestamp_ms: None,
                profile: None,
                verdict: "unknown".into(),
                action: "unknown".into(),
                report_ids: vec![],
                detections: vec![],
                detector_hints: vec![],
                sources: vec!["session-details".into()],
                notes: vec![],
            });
        }
    }
    Ok(events)
}

pub(super) fn report_ownership(
    conn: &Connection,
    descriptor: &super::work::Descriptor,
) -> Result<bool, CliError> {
    let Some(report) = &descriptor.report_id else {
        return Ok(true);
    };
    let id = digest(&descriptor.identity)?;
    let unique: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM event_reports WHERE report_id=?1 AND event_id=?2) AND NOT EXISTS(SELECT 1 FROM event_reports WHERE report_id=?1 AND event_id!=?2)",
            params![report, id],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    Ok(unique)
}

/// Debug-build synchronization only, following the process signal marker model.
/// No release binary contains the variable name or reads it.
#[cfg(debug_assertions)]
fn pause_after_batch(session: &scm::Session) {
    use std::os::unix::fs::OpenOptionsExt;
    let Some(path) = std::env::var_os("AIRS_CLI_TEST_HISTORY_BATCH_MARKER")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
    else {
        return;
    };
    if std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .is_err()
    {
        return;
    }
    let until = std::time::Instant::now() + Duration::from_secs(5);
    while path.exists() && !session.is_cancelled() && database::pause(until) {}
}
#[cfg(not(debug_assertions))]
fn pause_after_batch(_: &scm::Session) {}

fn store(conn: &Connection, record: &Record, run: &str, observed: i64) -> Result<(), CliError> {
    store_with_reconciliation(conn, record, run, observed, true)
}

fn store_with_reconciliation(
    conn: &Connection,
    record: &Record,
    run: &str,
    observed: i64,
    reconcile: bool,
) -> Result<(), CliError> {
    match record {
        Record::Event(original) => {
            let e = &original.event;
            let id = digest(&e.identity)?;
            // The fact keeps the ID of its received content (as a prune keeps it), and is
            // stored with every pruned excerpt erased: a repeated fact or a new variant
            // never brings erased data back.
            let fact_id = digest(original)?;
            let mut stored = original.clone();
            erase_tombstoned(conn, &id, &mut stored)?;
            let fact = &stored;
            conn.execute("INSERT OR IGNORE INTO events VALUES (?1,?2,?3,?4,?5,?6,NULL,NULL,?7,?7,NULL,'unknown','unknown',0)",params![id,e.identity.app_id,e.identity.app_name,e.identity.session_id,e.identity.scan_id,e.identity.sub_request.map(integer).transpose()?,observed]).map_err(sql_error)?;
            let inserted = conn
                .execute(
                    "INSERT OR IGNORE INTO event_facts VALUES (?1,?2,?3,?4,1,?5)",
                    params![fact_id, id, fact.origin, observed, encoded(fact)?],
                )
                .map_err(sql_error)?;
            let linked = conn
                .execute(
                    "INSERT OR IGNORE INTO fact_runs VALUES (?1,?2,?3)",
                    params![fact_id, run, observed],
                )
                .map_err(sql_error)?;
            // A service collection run counts its facts as they are written: new (this
            // write created the fact) or duplicate (the fact existed, newly linked). The
            // counts are read from that record, never from run timestamps (M4 review 13).
            if linked == 1 {
                let field = if inserted == 1 {
                    "$.fact_counts.new"
                } else {
                    "$.fact_counts.duplicate"
                };
                conn.execute(
                    "UPDATE collection_runs SET provenance_json=json_set(provenance_json,?2,json_extract(provenance_json,?2)+1) WHERE run_id=?1 AND json_type(provenance_json,'$.fact_counts')='object'",
                    params![run, field],
                )
                .map_err(sql_error)?;
            }
            conn.execute("UPDATE events SET first_seen_ms=min(first_seen_ms,?2),last_seen_ms=max(last_seen_ms,?2) WHERE event_id=?1",params![id,observed]).map_err(sql_error)?;
            examples(conn, &id, fact, run, observed)?;
            rebuild(conn, &id)?;
            if reconcile {
                reconcile_scan(conn, &e.identity.scan_id)?;
            }
        }
        Record::Receipt { origin, fact } => {
            let id = digest(origin)?;
            let fact_id = digest(&(origin, fact))?;
            conn.execute("INSERT OR IGNORE INTO receipts VALUES (?1,?2,?3,?3,NULL,NULL,NULL,NULL,NULL,NULL,'{\"identity_withheld\":false}','partial',NULL)",params![id,origin,observed]).map_err(sql_error)?;
            conn.execute(
                "INSERT OR IGNORE INTO receipt_facts VALUES (?1,?2,?3,?4)",
                params![fact_id, id, observed, encoded(fact)?],
            )
            .map_err(sql_error)?;
            conn.execute(
                "INSERT OR IGNORE INTO receipt_runs VALUES (?1,?2,?3)",
                params![fact_id, run, observed],
            )
            .map_err(sql_error)?;
            conn.execute("UPDATE receipts SET first_seen_ms=min(first_seen_ms,?2),last_seen_ms=max(last_seen_ms,?2) WHERE receipt_id=?1",params![id,observed]).map_err(sql_error)?;
            refresh_receipt(conn, &id, reconcile)?;
        }
        Record::Profile {
            profile,
            key,
            linked,
        } => {
            let id = digest(&(key, profile))?;
            let profile_id = key;
            conn.execute(
                "INSERT OR IGNORE INTO profile_observations VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![
                    id,
                    profile_id,
                    profile.name,
                    profile.revision.map(integer).transpose()?,
                    observed,
                    run,
                    encoded(&serde_json::json!({"profile":profile,"association":if *linked{"exact_name"}else{"unlinked_privacy"},"profile_key":key}))?
                ],
            )
            .map_err(sql_error)?;
            conn.execute(
                "INSERT OR IGNORE INTO profile_runs VALUES (?1,?2,?3)",
                params![id, run, observed],
            )
            .map_err(sql_error)?;
        }
        Record::Metric {
            metric,
            window,
            population_key,
            linked,
        } => {
            let id = digest(&(metric, window, population_key))?;
            conn.execute(
                "INSERT OR IGNORE INTO aggregate_observations VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    id,
                    run,
                    observed,
                    metric.source,
                    population_key,
                    metric.unit,
                    metric.scale,
                    window,
                    encoded(&serde_json::json!({"metric":metric,"association":if *linked{"exact_label"}else{"unlinked_privacy"},"population_key":population_key}))?
                ],
            )
            .map_err(sql_error)?;
            conn.execute(
                "INSERT OR IGNORE INTO aggregate_runs VALUES (?1,?2,?3)",
                params![id, run, observed],
            )
            .map_err(sql_error)?;
        }
        Record::Application {
            application,
            key,
            linked,
        } => {
            let id = digest(&(key, application))?;
            conn.execute("INSERT OR IGNORE INTO application_observations VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",params![id,key,if *linked{Some(&application.id)}else{None},if *linked{Some(&application.name)}else{None},application.profile,observed,run,encoded(&serde_json::json!({"application":application,"association":if *linked{"exact_identity"}else{"unlinked_privacy"},"application_key":key}))?]).map_err(sql_error)?;
            conn.execute(
                "INSERT OR IGNORE INTO application_runs VALUES (?1,?2,?3)",
                params![id, run, observed],
            )
            .map_err(sql_error)?;
        }
        Record::Coverage {
            source,
            since,
            until,
            selected_apps,
            scope_supported,
            native,
        } => {
            let id = digest(&(run, source, since, until, selected_apps))?;
            let state = match source.status {
                Status::Complete if *native => "traversed_as_of",
                Status::Complete => "observed",
                Status::Partial => "partial",
                Status::Unavailable => "unavailable",
            };
            let app = if *scope_supported && selected_apps.len() == 1 {
                selected_apps.first()
            } else {
                None
            };
            let scope_kind = if app.is_some() {
                "application"
            } else if *native && *scope_supported && selected_apps.is_empty() {
                "cloud"
            } else if *scope_supported {
                "selection"
            } else {
                "unknown_selection"
            };
            conn.execute("INSERT OR IGNORE INTO coverage_segments VALUES (?1,?2,?3,NULL,?4,?5,?6,?7,?8,?9)",params![id,run,app,source.name,since,until,observed,state,encoded(&serde_json::json!({"source":source,"selected_app_ids":selected_apps,"scope_kind":scope_kind,"scope_identity_supported":scope_supported,"coverage_semantics":if *native{"traversed as of collection; rolling bounds, late arrival and boundary uncertainty; no provider watermark"}else{"legacy observations do not establish tenant-wide traversal"}}))?]).map_err(sql_error)?;
        }
    }
    Ok(())
}

/// The example ID of one snippet of `event` (content-addressed: its excerpts included).
pub(crate) fn snippet_example_id(
    event: &str,
    detection: &airs_runtime_core::signal::evidence::Detection,
    snippet: &airs_runtime_core::signal::evidence::Snippet,
) -> Result<String, CliError> {
    digest(&(
        event,
        &detection.report_id,
        detection.row,
        &detection.detector,
        &detection.direction,
        snippet,
    ))
}

/// Enforce the archive's erasure records (`pruned_examples` tombstones) on one fact of
/// `event` before it is stored or promoted: every snippet whose content-addressed
/// example was pruned loses its excerpts (`withheld`), whichever copy carries it (a
/// repeated fact, a new variant, a promoted receipt). Returns whether anything was
/// erased.
fn erase_tombstoned(
    conn: &Connection,
    event: &str,
    fact: &mut EventFact,
) -> Result<bool, CliError> {
    use airs_runtime_core::signal::evidence::SnippetState;
    let mut erased = false;
    let mut detections = std::mem::take(&mut fact.event.detections);
    for detection in &mut detections {
        let mut snippets = std::mem::take(&mut detection.snippets);
        for snippet in &mut snippets {
            if snippet.excerpts.is_empty() {
                continue;
            }
            let id = snippet_example_id(event, detection, snippet)?;
            let gone: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM pruned_examples WHERE example_id=?1)",
                    [&id],
                    |r| r.get(0),
                )
                .map_err(sql_error)?;
            if gone {
                snippet.excerpts.clear();
                snippet.state = SnippetState::Withheld;
                erased = true;
            }
        }
        detection.snippets = snippets;
    }
    fact.event.detections = detections;
    Ok(erased)
}

/// Whether a stored fact (its `fields_json`) of `event` still carries an excerpt that
/// `tombstones` (a connection holding `pruned_examples`) records as pruned.
pub(crate) fn fact_carries_pruned(
    tombstones: &Connection,
    event: &str,
    fields_json: &str,
) -> Result<bool, CliError> {
    let mut fact: EventFact = serde_json::from_str(fields_json)
        .map_err(|_| failure("stored normalized fact is invalid"))?;
    erase_tombstoned(tombstones, event, &mut fact)
}

/// One derived example row: `(example_id, report_id, row, detector, direction, family,
/// state, excerpts_json)`.
pub(crate) type DerivedExample = (String, String, i64, String, String, String, String, String);
/// A fact's identity: `(event_id, app_id, app_name, session_id, scan_id, sub_request)`.
pub(crate) type FactIdentity = (String, String, String, String, String, Option<i64>);

/// The example rows `fields_json` (a fact of `event`) derives, as stored by
/// [`store`]: `(example_id, report_id, row, detector, direction, family, state,
/// excerpts_json)`.
pub(crate) fn derived_examples(
    event: &str,
    fields_json: &str,
) -> Result<Vec<DerivedExample>, CliError> {
    let fact: EventFact = serde_json::from_str(fields_json)
        .map_err(|_| failure("stored normalized fact is invalid"))?;
    let mut out = Vec::new();
    for detection in &fact.event.detections {
        for snippet in &detection.snippets {
            let state = encoded(&snippet.state)?;
            out.push((
                snippet_example_id(event, detection, snippet)?,
                detection.report_id.clone(),
                integer(detection.row)?,
                detection.detector.clone(),
                detection.direction.clone(),
                snippet.family.clone(),
                state.trim_matches('"').to_owned(),
                encoded(&snippet.excerpts)?,
            ));
        }
    }
    Ok(out)
}

/// The event ID (`digest` of the identity) and the identity columns a stored fact
/// names: `(event_id, app_id, app_name, session_id, scan_id, sub_request)`.
pub(crate) fn fact_identity(fields_json: &str) -> Result<FactIdentity, CliError> {
    let fact: EventFact = serde_json::from_str(fields_json)
        .map_err(|_| failure("stored normalized fact is invalid"))?;
    let i = &fact.event.identity;
    Ok((
        digest(i)?,
        i.app_id.clone(),
        i.app_name.clone(),
        i.session_id.clone(),
        i.scan_id.clone(),
        i.sub_request.map(integer).transpose()?,
    ))
}

/// Re-run receipt projection and per-scan reconciliation for `receipt` (the import's
/// projection check and its final rebuild).
/// The receipt ID the archive derives from an origin key (the digest of the key).
pub(crate) fn receipt_id_of(origin: &str) -> Result<String, CliError> {
    digest(&origin)
}

pub(crate) fn refresh_receipt_projection(conn: &Connection, receipt: &str) -> Result<(), CliError> {
    refresh_receipt(conn, receipt, true)
}

fn examples(
    conn: &Connection,
    event: &str,
    fact: &EventFact,
    run: &str,
    observed: i64,
) -> Result<(), CliError> {
    for detection in &fact.event.detections {
        for snippet in &detection.snippets {
            let id = snippet_example_id(event, detection, snippet)?;
            // A pruned example is never recreated (its tombstone is permanent).
            let gone: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM pruned_examples WHERE example_id=?1)",
                    [&id],
                    |r| r.get(0),
                )
                .map_err(sql_error)?;
            if gone {
                continue;
            }
            let state = encoded(&snippet.state)?;
            let state = state.trim_matches('"');
            conn.execute("INSERT OR IGNORE INTO examples VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,1,?10,?11)",params![id,event,detection.report_id,integer(detection.row)?,detection.detector,detection.direction,snippet.family,state,encoded(&snippet.excerpts)?,observed,encoded(&serde_json::json!({"origin":fact.origin,"scanner_endpoint":fact.scanner_endpoint}))?]).map_err(sql_error)?;
            conn.execute(
                "INSERT OR IGNORE INTO example_runs VALUES (?1,?2,?3)",
                params![id, run, observed],
            )
            .map_err(sql_error)?;
        }
    }
    Ok(())
}

fn facts(conn: &Connection, sql: &str, id: &str) -> Result<Vec<EventFact>, CliError> {
    facts_limited(conn, sql, id, &mut (16 * 1024 * 1024))
}
fn facts_limited(
    conn: &Connection,
    sql: &str,
    id: &str,
    remaining: &mut usize,
) -> Result<Vec<EventFact>, CliError> {
    let mut statement = conn.prepare(sql).map_err(sql_error)?;
    let mut cursor = statement.query([id]).map_err(sql_error)?;
    let mut out = Vec::new();
    while let Some(row) = cursor.next().map_err(sql_error)? {
        let text: String = row.get(0).map_err(sql_error)?;
        *remaining=remaining.checked_sub(text.len()).ok_or_else(||failure("event history exceeds the combined 16 MiB merge bound; existing facts were preserved"))?;
        out.push(
            serde_json::from_str(&text)
                .map_err(|_| failure("stored normalized fact is invalid"))?,
        );
    }
    Ok(out)
}

/// One event's projection, computed from a set of its retained facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Projection {
    pub event_ms: Option<i64>,
    pub profile: Option<String>,
    pub report_id: Option<String>,
    pub reports: Vec<String>,
    pub verdict: String,
    pub action: String,
    pub conflicting: bool,
    /// `(detector, direction, verdict, action, conflicting, provenance)`.
    pub detectors: Vec<(String, String, String, String, bool, serde_json::Value)>,
}

impl Projection {
    /// The detectors a summary counts as positive (the report summary's rule).
    pub(crate) fn positive_detectors(&self) -> BTreeSet<String> {
        self.detectors
            .iter()
            .filter(|(_, _, verdict, _, _, p)| {
                verdict == "malicious"
                    || p["positive_metadata"] == serde_json::Value::Bool(true)
                    || p["comparison_subsumed"] == serde_json::Value::Bool(true)
            })
            .map(|(detector, ..)| detector.clone())
            .collect()
    }
}

fn facts_until(
    conn: &Connection,
    sql: &str,
    id: &str,
    cutoff: Option<i64>,
    remaining: &mut usize,
) -> Result<Vec<EventFact>, CliError> {
    let mut statement = conn.prepare(sql).map_err(sql_error)?;
    let mut cursor = statement.query(params![id, cutoff]).map_err(sql_error)?;
    let mut out = Vec::new();
    while let Some(row) = cursor.next().map_err(sql_error)? {
        let text: String = row.get(0).map_err(sql_error)?;
        *remaining=remaining.checked_sub(text.len()).ok_or_else(||failure("event history exceeds the combined 16 MiB merge bound; existing facts were preserved"))?;
        out.push(
            serde_json::from_str(&text)
                .map_err(|_| failure("stored normalized fact is invalid"))?,
        );
    }
    Ok(out)
}

/// Event `id`'s projection from the facts first observed by `cutoff` (all facts when
/// `None`). Read-only: a report version reconstructs exactly what was known at its
/// evidence cutoff without touching the archive (M6 review 1 finding 1).
pub(crate) fn projection_as_of(
    conn: &Connection,
    id: &str,
    cutoff: Option<i64>,
) -> Result<Projection, CliError> {
    projection_with(conn, id, cutoff, &ReceiptsAsOf::current())
}

/// [`projection_as_of`] with receipt ownership as of the cutoff (`receipts`): a receipt
/// whose association changed after the cutoff contributes to the event it belonged to
/// then, never to its current one (M6 review 2 finding 1).
pub(crate) fn projection_with(
    conn: &Connection,
    id: &str,
    cutoff: Option<i64>,
    receipts: &ReceiptsAsOf,
) -> Result<Projection, CliError> {
    let mut remaining = 16 * 1024 * 1024;
    let mut all = facts_until(
        conn,
        "SELECT fields_json FROM event_facts WHERE event_id=?1 AND (?2 IS NULL OR observed_ms<=?2) ORDER BY fact_id",
        id,
        cutoff,
        &mut remaining,
    )?;
    all.extend(receipt_facts_until(
        conn,
        id,
        cutoff,
        receipts,
        &mut remaining,
    )?);
    project(&all)
}

/// Which receipts belonged to which event as of a cutoff, for the receipts whose
/// association could have changed after it (a receipt fact first observed after the
/// cutoff, or an event of its scan first seen, or given a fact, after it). Every other
/// receipt's current association is its association at the cutoff.
#[derive(Clone, Debug, Default)]
pub(crate) struct ReceiptsAsOf {
    /// Changed receipt → the event it was promoted to at the cutoff (`None`: partial,
    /// ambiguous, conflicting or not yet received).
    pub owner: BTreeMap<String, Option<String>>,
    /// Their current association (to find the events whose receipts differ).
    pub current: BTreeMap<String, Option<String>>,
}

impl ReceiptsAsOf {
    /// No override: current associations (the archive's own, cutoff-free reads).
    pub(crate) fn current() -> Self {
        Self::default()
    }

    fn changed_json(&self) -> String {
        serde_json::to_string(&self.owner.keys().collect::<Vec<_>>())
            .unwrap_or_else(|_| "[]".into())
    }

    fn owned_json(&self, event: &str) -> String {
        let ids: Vec<&String> = self
            .owner
            .iter()
            .filter(|(_, o)| o.as_deref() == Some(event))
            .map(|(r, _)| r)
            .collect();
        serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into())
    }

    /// Events whose receipts at the cutoff differ from their current ones.
    pub(crate) fn affected_events(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for (receipt, then) in &self.owner {
            let now = self.current.get(receipt).cloned().flatten();
            if *then != now {
                out.extend(then.clone());
                out.extend(now);
            }
        }
        out
    }
}

/// The receipt facts first observed by `cutoff` of the receipts that belonged to event
/// `id` then.
fn receipt_facts_until(
    conn: &Connection,
    id: &str,
    cutoff: Option<i64>,
    receipts: &ReceiptsAsOf,
    remaining: &mut usize,
) -> Result<Vec<EventFact>, CliError> {
    let mut statement = conn.prepare(RECEIPT_FACTS_AS_OF).map_err(sql_error)?;
    let mut cursor = statement
        .query(params![
            id,
            cutoff,
            receipts.changed_json(),
            receipts.owned_json(id)
        ])
        .map_err(sql_error)?;
    let mut out = Vec::new();
    while let Some(row) = cursor.next().map_err(sql_error)? {
        let text: String = row.get(0).map_err(sql_error)?;
        *remaining=remaining.checked_sub(text.len()).ok_or_else(||failure("event history exceeds the combined 16 MiB merge bound; existing facts were preserved"))?;
        out.push(
            serde_json::from_str(&text)
                .map_err(|_| failure("stored normalized fact is invalid"))?,
        );
    }
    Ok(out)
}

/// Receipt facts of event ?1 first observed by ?2: the receipts promoted to it now that
/// did not change after the cutoff (?3: changed receipts), plus the changed receipts that
/// belonged to it at the cutoff (?4).
const RECEIPT_FACTS_AS_OF: &str =
    "SELECT f.fields_json FROM receipt_facts f JOIN receipts r ON r.receipt_id=f.receipt_id
    WHERE ((r.promoted_event_id=?1 AND r.receipt_id NOT IN (SELECT value FROM json_each(?3)))
           OR r.receipt_id IN (SELECT value FROM json_each(?4)))
      AND (?2 IS NULL OR f.observed_ms<=?2) ORDER BY f.fact_id";

/// The receipts whose association may differ at `cutoff`, with their association then,
/// recomputed with [`reconcile_scan`]'s rule from what was known by the cutoff: the
/// receipt's facts first observed by then, and the events first seen (and their facts
/// first observed) by then.
pub(crate) fn receipts_as_of(conn: &Connection, cutoff: i64) -> Result<ReceiptsAsOf, CliError> {
    let changed: Vec<(String, Option<String>)> = conn
        .prepare(
            "SELECT r.receipt_id, r.promoted_event_id FROM receipts r
             WHERE EXISTS (SELECT 1 FROM receipt_facts f WHERE f.receipt_id=r.receipt_id AND f.observed_ms>?1)
                OR (r.scan_id IS NOT NULL AND EXISTS (SELECT 1 FROM events e WHERE e.scan_id=r.scan_id
                      AND (e.first_seen_ms>?1 OR EXISTS (SELECT 1 FROM event_facts ef WHERE ef.event_id=e.event_id AND ef.observed_ms>?1))))
             ORDER BY r.receipt_id",
        )
        .map_err(sql_error)?
        .query_map([cutoff], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(sql_error)?
        .collect::<rusqlite::Result<_>>()
        .map_err(sql_error)?;
    let mut out = ReceiptsAsOf::default();
    for (receipt, now) in changed {
        let then = receipt_owner_as_of(conn, &receipt, cutoff)?;
        out.owner.insert(receipt.clone(), then);
        out.current.insert(receipt, now);
    }
    Ok(out)
}

/// One receipt's association at `cutoff` ([`refresh_receipt`] and [`reconcile_scan`]
/// restricted to what was known by then).
fn receipt_owner_as_of(
    conn: &Connection,
    id: &str,
    cutoff: i64,
) -> Result<Option<String>, CliError> {
    let mut unbounded = usize::MAX;
    let all = facts_until(
        conn,
        "SELECT fields_json FROM receipt_facts WHERE receipt_id=?1 AND observed_ms<=?2 ORDER BY fact_id",
        id,
        Some(cutoff),
        &mut unbounded,
    )?;
    if all.is_empty() {
        return Ok(None);
    }
    let (mut scans, mut apps, mut names, mut sessions, mut requests, mut reports, mut endpoints) = (
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
    );
    let identity_withheld = all.iter().any(|f| f.identity_withheld);
    for f in &all {
        let e = &f.event;
        if uuid::Uuid::parse_str(&e.identity.scan_id).is_ok() {
            scans.insert(e.identity.scan_id.clone());
        }
        if !e.identity.app_id.is_empty() {
            apps.insert(e.identity.app_id.clone());
        }
        if !e.identity.app_name.is_empty() {
            names.insert(e.identity.app_name.clone());
        }
        if !e.identity.session_id.is_empty() {
            sessions.insert(e.identity.session_id.clone());
        }
        if let Some(n) = e.identity.sub_request {
            requests.insert(integer(n)?);
        }
        reports.extend(e.report_ids.iter().filter(|s| !withheld(s)).cloned());
        if let Some(endpoint) = &f.scanner_endpoint {
            endpoints.insert(endpoint.clone());
        }
    }
    let conflict = scans.len() > 1
        || apps.len() > 1
        || names.len() > 1
        || sessions.len() > 1
        || requests.len() > 1
        || reports.len() > 1
        || endpoints.len() > 1;
    let Some(scan) = scans.first() else {
        return Ok(None);
    };
    let (app, name, session) = (optional(&apps), optional(&names), optional(&sessions));
    let request = if requests.len() == 1 {
        requests.first().copied()
    } else {
        None
    };
    if conflict || identity_withheld || (app.is_none() && session.is_none() && request.is_none()) {
        return Ok(None);
    }
    let (report, endpoint) = (optional(&reports), optional(&endpoints));
    let matches: Vec<String> = conn
        .prepare(
            "SELECT e.event_id FROM events e WHERE e.scan_id=?1 AND e.first_seen_ms<=?8
               AND (?2 IS NULL OR e.app_id=?2) AND (?3 IS NULL OR e.app_name=?3)
               AND (?4 IS NULL OR e.session_id=?4) AND (?5 IS NULL OR e.sub_request=?5)
               AND (?6 IS NULL OR EXISTS(SELECT 1 FROM event_facts f,json_each(f.fields_json,'$.event.report_ids') r WHERE f.event_id=e.event_id AND f.observed_ms<=?8 AND r.value=?6))
               AND (SELECT count(DISTINCT json_extract(f.fields_json,'$.scanner_endpoint')) FROM event_facts f WHERE f.event_id=e.event_id AND f.observed_ms<=?8)<=1
               AND (?7 IS NULL OR EXISTS(SELECT 1 FROM event_facts f WHERE f.event_id=e.event_id AND f.observed_ms<=?8 AND json_extract(f.fields_json,'$.scanner_endpoint')=?7))
             ORDER BY e.event_id LIMIT 2",
        )
        .map_err(sql_error)?
        .query_map(
            params![scan, app, name, session, request, report, endpoint, cutoff],
            |r| r.get::<_, String>(0),
        )
        .map_err(sql_error)?
        .collect::<rusqlite::Result<_>>()
        .map_err(sql_error)?;
    Ok(if matches.len() == 1 {
        matches.into_iter().next()
    } else {
        None
    })
}

pub(crate) fn rebuild(conn: &Connection, id: &str) -> Result<(), CliError> {
    let p = projection_as_of(conn, id, None)?;
    conn.execute("UPDATE events SET event_ms=?2,profile=?3,report_id=?4,verdict=?5,action=?6,conflicting=?7 WHERE event_id=?1",params![id,p.event_ms,p.profile,p.report_id,p.verdict,p.action,i64::from(p.conflicting)]).map_err(sql_error)?;
    conn.execute("DELETE FROM event_reports WHERE event_id=?1", [id])
        .map_err(sql_error)?;
    for report in &p.reports {
        conn.execute(
            "INSERT INTO event_reports VALUES(?1,?2)",
            params![id, report],
        )
        .map_err(sql_error)?;
    }
    conn.execute("DELETE FROM detector_observations WHERE event_id=?1", [id])
        .map_err(sql_error)?;
    for (detector, direction, verdict, action, conflicting, provenance) in &p.detectors {
        conn.execute(
            "INSERT INTO detector_observations VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                id,
                detector,
                direction,
                verdict,
                action,
                i64::from(*conflicting),
                encoded(provenance)?
            ],
        )
        .map_err(sql_error)?;
    }
    Ok(())
}

fn project(all: &[EventFact]) -> Result<Projection, CliError> {
    let (mut verdicts, mut actions, mut profiles, mut reports, mut times) = (
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
    );
    let mut observations: BTreeMap<(String, String), DetectorProjection> = BTreeMap::new();
    for fact in all {
        // Client-asserted receipts (the service's `receipts_ingest`) are kept as their own
        // facts but never enter the authoritative projection: a client's claim cannot
        // change, or make conflicting, what the provider observed.
        if fact.origin == "client_asserted" {
            continue;
        }
        let e = &fact.event;
        known(&mut verdicts, &e.verdict);
        known(&mut actions, &e.action);
        if let Some(p) = &e.profile {
            profiles.insert(p.clone());
        }
        if let Some(at) = e.timestamp_ms {
            times.insert(at);
        }
        reports.extend(e.report_ids.iter().filter(|s| !withheld(s)).cloned());
        for hint in &e.detector_hints {
            if let Some((direction, detector)) = hint.split_once('/')
                && ["prompt", "response", "tool", "unknown"].contains(&direction)
            {
                let entry = observations
                    .entry((detectors::canonical(detector).into(), direction.into()))
                    .or_default();
                entry.positive_metadata = true;
                entry
                    .positive_hint_event_sources
                    .extend(e.sources.iter().cloned());
                entry.origins.insert(fact.origin.clone());
            }
        }
        for detection in &e.detections {
            let entry = observations
                .entry((
                    detectors::canonical(&detection.detector).into(),
                    detection.direction.clone(),
                ))
                .or_default();
            known(&mut entry.verdicts, &detection.verdict);
            known(&mut entry.actions, &detection.action);
            entry.report_observed = true;
            entry.report_event_sources.extend(e.sources.iter().cloned());
            entry.report_ids.insert(detection.report_id.clone());
            entry.origins.insert(fact.origin.clone());
        }
    }
    let failure_allows = verdicts.contains("error") && actions.contains("allow");
    let conflict = verdicts.len() > 1
        || actions.len() > 1
        || profiles.len() > 1
        || times.len() > 1
        || failure_allows;

    let directional: BTreeSet<String> = observations
        .iter()
        .filter(|((_, direction), value)| {
            direction != "unknown"
                && (value.positive_metadata || value.verdicts.contains("malicious"))
        })
        .map(|((detector, _), _)| detector.clone())
        .collect();
    for ((detector, direction), value) in &mut observations {
        if direction == "unknown" && directional.contains(detector) {
            value.comparison_subsumed =
                value.positive_metadata || value.verdicts.contains("malicious");
            value.positive_metadata = false;
            value.subsumed_hint_event_sources =
                std::mem::take(&mut value.positive_hint_event_sources);
        }
    }
    observations.retain(|(detector, direction), value| {
        direction != "unknown" || !directional.contains(detector) || value.report_observed
    });
    let detectors = observations
        .into_iter()
        .map(|((detector, direction), value)| {
            let provenance = serde_json::json!({"projection":"retained normalized facts","positive_metadata":value.positive_metadata,"positive_hint_event_sources":value.positive_hint_event_sources,"report_observed":value.report_observed,"report_event_sources":value.report_event_sources,"report_ids":value.report_ids,"origins":value.origins,"comparison_subsumed":value.comparison_subsumed,"subsumed_hint_event_sources":value.subsumed_hint_event_sources,"source_attribution":"event source sets; specific legacy hint attribution is unavailable"});
            (
                detector,
                direction,
                consensus(&value.verdicts, "unknown"),
                consensus(&value.actions, "unknown"),
                value.verdicts.len() > 1 || value.actions.len() > 1,
                provenance,
            )
        })
        .collect();
    Ok(Projection {
        event_ms: if times.len() == 1 {
            times.first().copied()
        } else {
            None
        },
        profile: optional(&profiles),
        report_id: optional(&reports),
        reports: reports.into_iter().collect(),
        verdict: consensus(&verdicts, "unknown"),
        action: if failure_allows {
            "unknown".into()
        } else {
            consensus(&actions, "unknown")
        },
        conflicting: conflict,
        detectors,
    })
}

fn refresh_receipt(conn: &Connection, id: &str, reconcile: bool) -> Result<(), CliError> {
    let all = facts(
        conn,
        "SELECT fields_json FROM receipt_facts WHERE receipt_id=?1 ORDER BY fact_id",
        id,
    )?;
    let (mut scans, mut apps, mut names, mut sessions, mut requests, mut reports, mut endpoints) = (
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
    );
    let mut references = BTreeSet::new();
    let identity_withheld = all.iter().any(|f| f.identity_withheld);
    for f in &all {
        let e = &f.event;
        if uuid::Uuid::parse_str(&e.identity.scan_id).is_ok() {
            scans.insert(e.identity.scan_id.clone());
        }
        if !e.identity.app_id.is_empty() {
            apps.insert(e.identity.app_id.clone());
        }
        if !e.identity.app_name.is_empty() {
            names.insert(e.identity.app_name.clone());
        }
        if !e.identity.session_id.is_empty() {
            sessions.insert(e.identity.session_id.clone());
        }
        if let Some(n) = e.identity.sub_request {
            requests.insert(integer(n)?);
        }
        reports.extend(e.report_ids.iter().filter(|s| !withheld(s)).cloned());
        if let Some(endpoint) = &f.scanner_endpoint {
            endpoints.insert(endpoint.clone());
        }
        if let Some(reference) = &f.scanner_reference {
            references.insert(encoded(reference)?);
        }
    }
    let prior: Option<String> = conn
        .query_row(
            "SELECT promoted_event_id FROM receipts WHERE receipt_id=?1",
            [id],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    let conflict = scans.len() > 1
        || apps.len() > 1
        || names.len() > 1
        || sessions.len() > 1
        || requests.len() > 1
        || reports.len() > 1
        || endpoints.len() > 1;
    let references = references
        .into_iter()
        .map(|text| {
            serde_json::from_str::<serde_json::Value>(&text)
                .map_err(|_| failure("invalid scanner reference"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    conn.execute("UPDATE receipts SET scan_id=?2,app_id=?3,app_name=?4,session_id=?5,sub_request=?6,report_id=?7,binding_json=?8,state=?9,promoted_event_id=NULL WHERE receipt_id=?1",params![id,optional(&scans),optional(&apps),optional(&names),optional(&sessions),if requests.len()==1{requests.first().copied()}else{None},optional(&reports),encoded(&serde_json::json!({"scanner_endpoint":optional(&endpoints),"scanner_references":references,"identity_withheld":identity_withheld}))?,if conflict{"conflicting"}else{"partial"}]).map_err(sql_error)?;
    if let Some(prior) = prior {
        rebuild(conn, &prior)?;
    }
    if reconcile
        && !conflict
        && let Some(scan) = scans.first()
    {
        reconcile_scan(conn, scan)?;
    }
    Ok(())
}

fn reconcile_scan(conn: &Connection, scan: &str) -> Result<(), CliError> {
    let mut after = String::new();
    let mut affected = BTreeSet::new();
    loop {
        let mut statement=conn.prepare("SELECT receipt_id,app_id,app_name,session_id,sub_request,report_id,binding_json,promoted_event_id FROM receipts WHERE scan_id=?1 AND state!='conflicting' AND receipt_id>?2 ORDER BY receipt_id LIMIT 100").map_err(sql_error)?;
        let receipts = statement
            .query_map(params![scan, after], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, Option<String>>(7)?,
                ))
            })
            .map_err(sql_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sql_error)?;
        drop(statement);
        if receipts.is_empty() {
            break;
        }
        for (id, app, name, session, request, report, binding, prior) in receipts {
            after = id.clone();
            let mut matches = Vec::new();
            let binding: serde_json::Value = serde_json::from_str(&binding)
                .map_err(|_| failure("invalid receipt binding metadata"))?;
            if (app.is_some() || session.is_some() || request.is_some())
                && binding["identity_withheld"] != true
            {
                let endpoint = binding["scanner_endpoint"].as_str();
                // LIMIT applies after every identity/report/endpoint predicate,
                // proving uniqueness without materializing a scan's whole group.
                let sql="SELECT e.event_id FROM events e WHERE e.scan_id=?1
                    AND (?2 IS NULL OR e.app_id=?2) AND (?3 IS NULL OR e.app_name=?3)
                    AND (?4 IS NULL OR e.session_id=?4) AND (?5 IS NULL OR e.sub_request=?5)
                    AND (?6 IS NULL OR EXISTS(SELECT 1 FROM event_facts f,json_each(f.fields_json,'$.event.report_ids') r WHERE f.event_id=e.event_id AND r.value=?6))
                    AND (SELECT count(DISTINCT json_extract(f.fields_json,'$.scanner_endpoint')) FROM event_facts f WHERE f.event_id=e.event_id)<=1
                    AND (?7 IS NULL OR EXISTS(SELECT 1 FROM event_facts f WHERE f.event_id=e.event_id AND json_extract(f.fields_json,'$.scanner_endpoint')=?7))
                    ORDER BY e.event_id LIMIT 2";
                matches = conn
                    .prepare(sql)
                    .map_err(sql_error)?
                    .query_map(
                        params![scan, app, name, session, request, report, endpoint],
                        |r| r.get::<_, String>(0),
                    )
                    .map_err(sql_error)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(sql_error)?;
            }
            let state = match matches.len() {
                0 => "partial",
                1 => "promoted",
                _ => "ambiguous",
            };
            let target = if matches.len() == 1 {
                matches.first().cloned()
            } else {
                None
            };
            if prior != target {
                if let Some(prior) = prior {
                    affected.insert(prior);
                }
                if let Some(target) = &target {
                    affected.insert(target.clone());
                }
            }
            conn.execute(
                "UPDATE receipts SET state=?2,promoted_event_id=?3 WHERE receipt_id=?1",
                params![id, state, target],
            )
            .map_err(sql_error)?;
            // A promoted receipt's facts join the event: its erasure records apply.
            if let Some(target) = &target {
                erase_receipt_facts(conn, &id, target)?;
            }
            if affected.len() >= 2000 {
                for event in std::mem::take(&mut affected) {
                    rebuild(conn, &event)?;
                }
            }
        }
    }
    for event in affected {
        rebuild(conn, &event)?;
    }
    Ok(())
}

/// Erase, in every fact of receipt `receipt`, the excerpts `event` has pruned.
fn erase_receipt_facts(conn: &Connection, receipt: &str, event: &str) -> Result<(), CliError> {
    let rows: Vec<(String, String)> = conn
        .prepare("SELECT fact_id, fields_json FROM receipt_facts WHERE receipt_id=?1")
        .map_err(sql_error)?
        .query_map([receipt], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(sql_error)?
        .collect::<rusqlite::Result<_>>()
        .map_err(sql_error)?;
    for (fact_id, fields) in rows {
        let mut fact: EventFact = serde_json::from_str(&fields)
            .map_err(|_| failure("stored normalized fact is invalid"))?;
        if erase_tombstoned(conn, event, &mut fact)? {
            conn.execute(
                "UPDATE receipt_facts SET fields_json=?2 WHERE fact_id=?1",
                params![fact_id, encoded(&fact)?],
            )
            .map_err(sql_error)?;
        }
    }
    Ok(())
}

/// Metadata already received by a CLI scanner operation. Every entry remains a
/// receipt: SDK scanning responses do not establish the exact management app name.
pub struct Received {
    pub receipt_key: String,
    pub batch_report_id: Option<String>,
    pub event: Event,
    pub source: &'static str,
    pub endpoint: String,
    pub reference: Option<airs_runtime_core::tenant::scanners::ScannerReference>,
}

fn received_record(
    input: &Received,
    secrets: &SecretSet,
    payload: &PayloadSpans,
) -> Result<Record, CliError> {
    if input.receipt_key.is_empty()
        || input.receipt_key.len() > 512
        || input.receipt_key.chars().any(char::is_control)
        || secrets.carried_by(&input.receipt_key)
        || ![
            "cli_scan",
            "cli_submit",
            "cli_results",
            "cli_reports",
            "client_asserted",
        ]
        .contains(&input.source)
        || !input.event.identity.app_name.is_empty()
        || input.event.timestamp_ms.is_some()
        || input
            .event
            .detections
            .iter()
            .any(|d| !d.snippets.is_empty())
        || !["benign", "malicious", "unknown", "error"].contains(&input.event.verdict.as_str())
        || !["allow", "block", "unknown", "failed"].contains(&input.event.action.as_str())
        || input
            .event
            .identity
            .sub_request
            .is_some_and(|n| i64::try_from(n).is_err())
    {
        return Err(failure("received scan metadata is unsupported"));
    }
    if secrets.carried_by(&input.endpoint) {
        return Err(failure("scanner endpoint cannot be safely retained"));
    }
    let admitted = airs_runtime_core::clients::admit(
        "airsApiEndpoint",
        Some(&input.endpoint),
        &input.endpoint,
    )?;
    if admitted.url().as_str().trim_end_matches('/') != input.endpoint {
        return Err(failure("scanner endpoint is not canonical"));
    }
    let mut tree = serde_json::to_value(&input.event)
        .map_err(|_| failure("cannot normalize received metadata"))?;
    privacy::document(&mut tree, secrets, payload);
    let mut event: Event = serde_json::from_value(tree)
        .map_err(|_| failure("received metadata cannot be safely retained"))?;
    let mut identity_withheld = input.event.identity != event.identity
        || [
            &event.identity.app_id,
            &event.identity.app_name,
            &event.identity.session_id,
            &event.identity.scan_id,
        ]
        .iter()
        .any(|s| withheld(s))
        || input.event.report_ids != event.report_ids
        || input.event.report_ids.iter().any(|s| withheld(s));
    if !["benign", "malicious", "unknown", "error"].contains(&event.verdict.as_str()) {
        event.verdict = "unknown".into();
    }
    if !["allow", "block", "unknown", "failed"].contains(&event.action.as_str()) {
        event.action = "unknown".into();
    }
    event
        .detector_hints
        .retain(|hint| !withheld(hint) && input.event.detector_hints.contains(hint));
    event.detections.retain_mut(|d| {
        let Some(original) = input.event.detections.iter().find(|o| o.row == d.row) else {
            return false;
        };
        if d.detector != original.detector
            || withheld(&d.detector)
            || d.report_id != original.report_id
            || withheld(&d.report_id)
        {
            return false;
        }
        if !["prompt", "response", "tool", "unknown"].contains(&d.direction.as_str()) {
            d.direction = "unknown".into();
        }
        if !["benign", "malicious", "unknown", "error"].contains(&d.verdict.as_str()) {
            d.verdict = "unknown".into();
        }
        if !["allow", "block", "unknown", "failed"].contains(&d.action.as_str()) {
            d.action = "unknown".into();
        }
        true
    });
    for (before, after) in [
        (&input.event.identity.app_id, &mut event.identity.app_id),
        (
            &input.event.identity.session_id,
            &mut event.identity.session_id,
        ),
        (&input.event.identity.scan_id, &mut event.identity.scan_id),
    ] {
        if before != after || withheld(after) {
            after.clear();
        }
    }
    if event.profile != input.event.profile || event.profile.as_ref().is_some_and(|s| withheld(s)) {
        event.profile = None;
    }
    event
        .report_ids
        .retain(|id| !id.trim().is_empty() && !withheld(id) && input.event.report_ids.contains(id));
    event.report_ids.sort();
    event.report_ids.dedup();
    if let Ok(id) = uuid::Uuid::parse_str(&event.identity.scan_id) {
        event.identity.scan_id = id.to_string();
    } else {
        event.identity.scan_id.clear();
    }
    let reference = if let Some(reference) = &input.reference {
        let mut tree =
            serde_json::to_value(reference).map_err(|_| failure("invalid scanner reference"))?;
        // This is the admitted public endpoint, not server text. IP literals are
        // part of its exact namespace; credentials/userinfo/queries were refused
        // above. Project labels without turning a loopback/custom IP into an alias.
        tree.as_object_mut()
            .ok_or_else(|| failure("invalid scanner reference"))?
            .remove("endpoint");
        privacy::document(&mut tree, secrets, payload);
        tree["endpoint"] = serde_json::Value::String(reference.endpoint.clone());
        let safe: Option<airs_runtime_core::tenant::scanners::ScannerReference> =
            serde_json::from_value(tree).ok();
        safe.filter(|r| {
            r == reference
                && r.valid()
                && r.application_id == event.identity.app_id
                && r.endpoint == input.endpoint
        })
    } else {
        None
    };
    // Application attribution comes only from an intact explicit binding.
    if reference.is_none() {
        identity_withheld |= !event.identity.app_id.is_empty();
        event.identity.app_id.clear();
    }
    // Client-asserted receipts (the service's `receipts_ingest`) have their own origin
    // namespace, so they can never collide with a CLI receipt key.
    let namespace = if input.source == "client_asserted" {
        "mcp"
    } else {
        "cli"
    };
    let origin = format!("{namespace}/{}", digest(&input.receipt_key)?);
    let batch_report_id = input
        .batch_report_id
        .as_ref()
        .filter(|s| s.len() <= 4096)
        .filter(|s| {
            privacy::text_with_spans(s, secrets, payload, "report_id", 4096) == **s && !withheld(s)
        })
        .cloned();
    let fact = EventFact {
        event,
        identity_withheld,
        origin: input.source.into(),
        scanner_endpoint: Some(input.endpoint.clone()),
        scanner_reference: reference,
        batch_report_id,
    };
    let record = Record::Receipt { origin, fact };
    if record.bytes()? > 256 * 1024 {
        return Err(failure("received metadata exceeds the bounded write size"));
    }
    Ok(record)
}

/// One cumulative250ms local batch. No network/writer lease and no remote work.
/// A fresh local session permits already-received progress to survive a network
/// cancellation while this short persistence deadline still bounds cleanup.
/// The explicit examples-only prune (plan "History transfer and lifecycle"): every
/// live copy of the excerpts of events before `before_ms` (for `apps`, or every
/// application when empty) is removed, and nothing else changes.
///
/// - Preview (`force == false`) counts what would change and writes nothing.
/// - Execute removes the excerpts from the event facts, the receipt facts promoted to
///   those events and the `examples` rows; records a tombstone per example; rebuilds
///   the projections (verdicts, actions and metadata stay); and compacts the file with
///   `VACUUM` under `secure_delete`, so no freed page keeps a copy.
///
/// Earlier logical exports and external backups are not erased (no forensic claim).
pub fn prune_examples(
    root: &Path,
    cloud: &Cloud,
    before_ms: i64,
    apps: &[String],
    force: bool,
    budget: std::time::Duration,
    session: &scm::Session,
) -> Result<serde_json::Value, CliError> {
    prune_examples_inner(root, cloud, before_ms, apps, force, None, budget, session)
}

/// [`prune_examples`] (forced) bound to the mutation generation its preview saw: the
/// binding is checked inside the pruning write transaction, so an archive changed since
/// the preview (any write, even inside an existing run) is refused before any erasure.
pub fn prune_examples_bound(
    root: &Path,
    cloud: &Cloud,
    before_ms: i64,
    apps: &[String],
    expected_generation: i64,
    budget: std::time::Duration,
    session: &scm::Session,
) -> Result<serde_json::Value, CliError> {
    prune_examples_inner(
        root,
        cloud,
        before_ms,
        apps,
        true,
        Some(expected_generation),
        budget,
        session,
    )
}

/// The refusal of an operation whose preview saw another archive generation.
pub const GENERATION_CHANGED: &str =
    "the archive changed since the preview; preview again before executing";

#[allow(clippy::too_many_arguments)]
fn prune_examples_inner(
    root: &Path,
    cloud: &Cloud,
    before_ms: i64,
    apps: &[String],
    force: bool,
    expected_generation: Option<i64>,
    budget: std::time::Duration,
    session: &scm::Session,
) -> Result<serde_json::Value, CliError> {
    let apps_json = encoded(&apps)?;
    // The selection is always expressed in SQL (a subquery, or a temporary table under
    // the writer), never as one bound list of IDs: an ordinary large selection stays
    // within SQLite's 1 MiB value limit and is never materialised in memory (M5 review
    // 3 finding 6).
    let select = "SELECT event_id FROM events WHERE event_ms IS NOT NULL AND event_ms < ?1 AND (json_array_length(?2)=0 OR app_id IN (SELECT value FROM json_each(?2)))";
    let count = |conn: &Connection,
                 selection: &str,
                 args: &[&dyn rusqlite::ToSql]|
     -> Result<(i64, i64, i64, i64), CliError> {
        let q = |sql: String| -> Result<i64, CliError> {
            conn.query_row(&sql, args, |r| r.get(0)).map_err(sql_error)
        };
        Ok((
            q(format!("SELECT count(*) FROM ({selection})"))?,
            q(format!(
                "SELECT count(*) FROM examples WHERE event_id IN ({selection})"
            ))?,
            q(format!(
                "SELECT count(*) FROM event_facts WHERE event_id IN ({selection}) AND instr(fields_json,'\"excerpts\":[\"')>0"
            ))?,
            q(format!(
                "SELECT count(*) FROM receipt_facts f JOIN receipts r USING(receipt_id) WHERE r.promoted_event_id IN ({selection}) AND instr(f.fields_json,'\"excerpts\":[\"')>0"
            ))?,
        ))
    };
    if !force {
        let Some(read) = Reader::open(root, cloud, session)? else {
            return Err(failure("archive is missing; prune never initializes it"));
        };
        let (events, examples, event_facts, receipt_facts) =
            count(&read.conn, select, &[&before_ms, &apps_json])?;
        // The counts and the generation come from one read transaction, so a preview's
        // displayed counts describe exactly the binding it admits.
        let generation = read.generation()?;
        return Ok(
            serde_json::json!({"kind":"runtime_history_prune","mode":"preview","scope":"examples_only","events":events,"examples":examples,"event_facts_with_excerpts":event_facts,"receipt_facts_with_excerpts":receipt_facts,"writes":0,"network_requests":0,"erases_external_copies":false,"generation":generation}),
        );
    }
    let w = Writer::existing(root, cloud, budget, session)?;
    w.conn.execute_batch("BEGIN IMMEDIATE").map_err(sql_error)?;
    if let Some(expected) = expected_generation
        && !w.bound_to(expected)?
    {
        let _ = w.conn.execute_batch("ROLLBACK");
        return Err(failure(GENERATION_CHANGED));
    }
    let result = (|| {
        select_into_temp(&w.conn)?;
        w.conn
            .execute(
                &format!("INSERT OR IGNORE INTO temp.prune_selection {select}"),
                params![before_ms, apps_json],
            )
            .map_err(sql_error)?;
        let (events, examples, event_facts, receipt_facts) = count(&w.conn, SELECTED, &[])?;
        let tombstones = erase_selected(&w.conn, database::now_ms()?, &|| w.check())?;
        w.check()?;
        Ok::<_, CliError>((events, examples, event_facts, receipt_facts, tombstones))
    })();
    let (events, examples, event_facts, receipt_facts, tombstones) = match result {
        Ok(v) => v,
        Err(e) => {
            let _ = w.conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    };
    w.conn.execute_batch("COMMIT").map_err(sql_error)?;
    w.check()?;
    drop(w);
    // No freed page keeps a copy: the archive is compacted (a verified `VACUUM INTO`
    // copy published in its place) before the prune reports success.
    database::compact(root, cloud, budget, session)?;
    Ok(
        serde_json::json!({"kind":"runtime_history_prune","mode":"execute","scope":"examples_only","events":events,"examples":examples,"event_facts_with_excerpts":event_facts,"receipt_facts_with_excerpts":receipt_facts,"tombstones":tombstones,"network_requests":0,"erases_external_copies":false}),
    )
}

/// The selected events of a prune, as SQL (the temporary selection table).
const SELECTED: &str = "SELECT event_id FROM temp.prune_selection";

/// A fresh, empty temporary selection table on the writer's connection.
fn select_into_temp(conn: &Connection) -> Result<(), CliError> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS temp.prune_selection;
         CREATE TEMP TABLE prune_selection (event_id TEXT PRIMARY KEY) WITHOUT ROWID;",
    )
    .map_err(sql_error)
}

/// Visit `sql`'s rows (`key`, payload) a bounded keyset page at a time (no whole-set
/// materialisation, and no cursor open while `visit` writes). `sql` must select its key
/// as the first column, filter `?1 IS NULL OR key > ?1`, order by it and `LIMIT 500`.
fn paged(
    conn: &Connection,
    sql: &str,
    visit: &mut dyn FnMut(&str, &str) -> Result<(), CliError>,
) -> Result<(), CliError> {
    let mut last: Option<String> = None;
    loop {
        let page: Vec<(String, String)> = conn
            .prepare(sql)
            .map_err(sql_error)?
            .query_map([&last], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(sql_error)?
            .collect::<rusqlite::Result<_>>()
            .map_err(sql_error)?;
        let Some((end, _)) = page.last().cloned() else {
            return Ok(());
        };
        for (key, payload) in &page {
            visit(key, payload)?;
        }
        last = Some(end);
    }
}

/// [`erase_selected`] for an explicit list of event IDs (loaded into the temporary
/// selection table in bounded chunks).
#[cfg(test)]
pub(crate) fn erase_events(
    conn: &Connection,
    ids: &[String],
    now: i64,
    check: &dyn Fn() -> Result<(), CliError>,
) -> Result<u64, CliError> {
    select_into_temp(conn)?;
    for chunk in ids.chunks(100) {
        conn.execute(
            "INSERT OR IGNORE INTO temp.prune_selection SELECT value FROM json_each(?1)",
            [encoded(&chunk)?],
        )
        .map_err(sql_error)?;
    }
    erase_selected(conn, now, check)
}

/// The erasure of an examples-only prune, inside the caller's write transaction:
/// tombstones for every excerpt any copy carries (the examples rows, every event fact
/// and every promoted receipt fact of `ids`, including snippets no examples row holds),
/// then every example row removed, every copy stripped and the events rebuilt (M5 review
/// 2 finding 2). A tombstone is the snippet's content-addressed example ID under the
/// event the copy belongs to (a receipt fact's promoted event), which is exactly what
/// [`erase_tombstoned`] checks when a fact is stored or a receipt promoted.
pub(crate) fn erase_selected(
    conn: &Connection,
    now: i64,
    check: &dyn Fn() -> Result<(), CliError>,
) -> Result<u64, CliError> {
    let before: i64 = conn
        .query_row("SELECT count(*) FROM pruned_examples", [], |r| r.get(0))
        .map_err(sql_error)?;
    conn.execute(
        &format!("INSERT OR IGNORE INTO pruned_examples SELECT example_id, ?1 FROM examples WHERE event_id IN ({SELECTED})"),
        params![now],
    )
    .map_err(sql_error)?;
    // Tombstones for every excerpt any fact copy carries (event facts and promoted
    // receipt facts), a page at a time.
    let tombstone = &mut |event: &str, fields: &str| -> Result<(), CliError> {
        check()?;
        let fact: EventFact = serde_json::from_str(fields)
            .map_err(|_| failure("stored normalized fact is invalid"))?;
        for detection in &fact.event.detections {
            for snippet in &detection.snippets {
                if !snippet.excerpts.is_empty() {
                    conn.execute(
                        "INSERT OR IGNORE INTO pruned_examples VALUES (?1, ?2)",
                        params![snippet_example_id(event, detection, snippet)?, now],
                    )
                    .map_err(sql_error)?;
                }
            }
        }
        Ok(())
    };
    // Keyed by fact ID (unique), carrying the copy's event and fields.
    for sql in [
        (format!(
            "SELECT fact_id, event_id || char(31) || fields_json FROM event_facts WHERE event_id IN ({SELECTED}) AND (?1 IS NULL OR fact_id > ?1) ORDER BY fact_id LIMIT 500"
        )),
        (format!(
            "SELECT f.fact_id, r.promoted_event_id || char(31) || f.fields_json FROM receipt_facts f JOIN receipts r USING(receipt_id) WHERE r.promoted_event_id IN ({SELECTED}) AND (?1 IS NULL OR f.fact_id > ?1) ORDER BY f.fact_id LIMIT 500"
        )),
    ] {
        paged(conn, &sql, &mut |_, joined| {
            let (event, fields) = joined
                .split_once('\u{1f}')
                .ok_or_else(|| failure("stored fact copy is invalid"))?;
            tombstone(event, fields)
        })?;
    }
    conn.execute(
        &format!("DELETE FROM examples WHERE event_id IN ({SELECTED})"),
        [],
    )
    .map_err(sql_error)?;
    // Strip excerpts from every fact copy (event facts and promoted receipt facts).
    let strip = |table: &str, sql: String| -> Result<(), CliError> {
        paged(conn, &sql, &mut |fact, fields| {
            check()?;
            let mut v: serde_json::Value = serde_json::from_str(fields)
                .map_err(|_| failure("archived fact is not valid JSON"))?;
            if let Some(detections) = v["event"]["detections"].as_array_mut() {
                for d in detections {
                    if let Some(snippets) = d["snippets"].as_array_mut() {
                        for sn in snippets {
                            if sn["excerpts"].as_array().is_some_and(|e| !e.is_empty()) {
                                sn["excerpts"] = serde_json::json!([]);
                                sn["state"] = serde_json::json!("withheld");
                            }
                        }
                    }
                }
            }
            conn.execute(
                &format!("UPDATE {table} SET fields_json=?2 WHERE fact_id=?1"),
                params![fact, v.to_string()],
            )
            .map_err(sql_error)?;
            Ok(())
        })
    };
    strip(
        "event_facts",
        format!(
            "SELECT fact_id, fields_json FROM event_facts WHERE event_id IN ({SELECTED}) AND (?1 IS NULL OR fact_id > ?1) ORDER BY fact_id LIMIT 500"
        ),
    )?;
    strip(
        "receipt_facts",
        format!(
            "SELECT f.fact_id, f.fields_json FROM receipt_facts f JOIN receipts r USING(receipt_id) WHERE r.promoted_event_id IN ({SELECTED}) AND (?1 IS NULL OR f.fact_id > ?1) ORDER BY f.fact_id LIMIT 500"
        ),
    )?;
    paged(
        conn,
        &format!(
            "SELECT event_id, '' FROM ({SELECTED}) WHERE ?1 IS NULL OR event_id > ?1 ORDER BY event_id LIMIT 500"
        ),
        &mut |id, _| {
            check()?;
            rebuild(conn, id)
        },
    )?;
    let after: i64 = conn
        .query_row("SELECT count(*) FROM pruned_examples", [], |r| r.get(0))
        .map_err(sql_error)?;
    conn.execute_batch("DROP TABLE IF EXISTS temp.prune_selection")
        .map_err(sql_error)?;
    Ok((after - before).max(0) as u64)
}

pub fn retain_received(
    root: &Path,
    cloud: &Cloud,
    run: &str,
    kind: &str,
    inputs: &[Received],
    secrets: &SecretSet,
    payload: &PayloadSpans,
) -> Result<(), CliError> {
    retain_received_as(
        root,
        cloud,
        run,
        kind,
        inputs,
        (secrets, payload),
        "cli_received_metadata",
    )
    .map(|_| ())
}

/// The service's `receipts_ingest` (contract §4.1): one bounded batch of normalized,
/// client-asserted receipts, through the same conservative normalization, privacy
/// projection and per-scan reconciliation as CLI receipts, recorded as a `scan` run with
/// provenance `client_asserted`. Returns `(accepted, duplicates)`: receipts new to the
/// archive and receipts whose origin key it already held.
pub fn ingest_client_receipts(
    root: &Path,
    cloud: &Cloud,
    run: &str,
    inputs: &[Received],
    secrets: &SecretSet,
    payload: &PayloadSpans,
) -> Result<(u64, u64), CliError> {
    if inputs.iter().any(|r| r.source != "client_asserted") {
        return Err(failure("received batch exceeds retention limits"));
    }
    retain_received_as(
        root,
        cloud,
        run,
        "scan",
        inputs,
        (secrets, payload),
        "client_asserted",
    )
}

fn retain_received_as(
    root: &Path,
    cloud: &Cloud,
    run: &str,
    kind: &str,
    inputs: &[Received],
    (secrets, payload): (&SecretSet, &PayloadSpans),
    origin_label: &str,
) -> Result<(u64, u64), CliError> {
    if inputs.is_empty() {
        return Ok((0, 0));
    }
    if inputs.len() > 200
        || !["scan", "bulk", "resume"].contains(&kind)
        || uuid::Uuid::parse_str(run).is_err()
        || payload.exceeded()
    {
        return Err(failure("received batch exceeds retention limits"));
    }
    cloud.check_public(secrets)?;
    let records = inputs
        .iter()
        .map(|r| received_record(r, secrets, payload))
        .collect::<Result<Vec<_>, _>>()?;
    let sizes = records
        .iter()
        .map(Record::bytes)
        .collect::<Result<Vec<_>, _>>()?;
    const BOUND: usize = 256 * 1024;
    // The CLI's batch is one bounded transaction. A service batch (`receipts_ingest`,
    // up to 200 records whose *input* is ≤ 256 KiB) normalizes larger, so it is written
    // in consecutive transactions each within the same 256 KiB bound, under one
    // deterministic run: a replay after a crash between them fills in the rest
    // idempotently (origin keys are unique; counts come from the run's own record).
    let chunks: Vec<std::ops::Range<usize>> = if origin_label == "client_asserted" {
        let mut out = Vec::new();
        let (mut start, mut used) = (0usize, 0usize);
        for (i, size) in sizes.iter().enumerate() {
            if *size > BOUND {
                return Err(failure("received batch exceeds retention limits"));
            }
            if used + size > BOUND {
                out.push(start..i);
                start = i;
                used = 0;
            }
            used += size;
        }
        out.push(start..records.len());
        out
    } else {
        if sizes.iter().sum::<usize>() > BOUND {
            return Err(failure("received batch exceeds retention limits"));
        }
        std::iter::once(0..records.len()).collect()
    };
    let session = scm::Session::new();
    // The CLI keeps its 250 ms local retention budget; the service's receipt batches run
    // off the request runtime inside a 10 s submission budget, so they get a larger,
    // still bounded, archive write budget.
    let budget = if origin_label == "client_asserted" {
        SERVICE_RECEIPT_BUDGET
    } else {
        database::RETAIN_BUDGET
    };
    let (mut fresh, mut repeated) = (0u64, 0u64);
    for range in chunks {
        let (f, r) = retain_chunk(
            root,
            cloud,
            run,
            kind,
            &records[range],
            budget,
            origin_label,
            &session,
        )?;
        fresh += f;
        repeated += r;
    }
    Ok((fresh, repeated))
}

/// One bounded transaction of a received batch: the run row, its records and their
/// per-scan reconciliation.
#[allow(clippy::too_many_arguments)]
fn retain_chunk(
    root: &Path,
    cloud: &Cloud,
    run: &str,
    kind: &str,
    records: &[Record],
    budget: std::time::Duration,
    origin_label: &str,
    session: &scm::Session,
) -> Result<(u64, u64), CliError> {
    let w = Writer::open(root, cloud, budget, false, session)?;
    let observed = database::now_ms()?;
    w.conn.execute_batch("BEGIN IMMEDIATE").map_err(sql_error)?;
    // A service receipt run records, in the transaction that inserts them, which
    // receipts it accepted first (`first_accepted`): its counts are read from that
    // immutable record, never re-derived from run timestamps (M4 review 12).
    let provenance = if origin_label == "client_asserted" {
        serde_json::json!({"origin":origin_label,"scope":"already_received_only","provider_completeness":false,"redaction_version":1,"sdk_version":"0.2.4","first_accepted":[]})
    } else {
        serde_json::json!({"origin":origin_label,"scope":"already_received_only","provider_completeness":false,"redaction_version":1,"sdk_version":"0.2.4"})
    };
    w.conn
        .execute(
            "INSERT OR IGNORE INTO collection_runs VALUES (?1,?2,?3,NULL,NULL,NULL,'running',?4)",
            params![run, kind, observed, encoded(&provenance)?],
        )
        .map_err(sql_error)?;
    if origin_label == "client_asserted" {
        // An existing run without the record: an empty one starts a complete (empty)
        // record; one already holding receipts is never extended, since a record begun
        // now would omit the acceptances made before it (M4 review 13).
        let (recorded, members): (bool, i64) = w
            .conn
            .query_row(
                "SELECT coalesce(json_type(provenance_json,'$.first_accepted')='array',0), (SELECT count(*) FROM receipt_runs WHERE run_id=?1) FROM collection_runs WHERE run_id=?1",
                params![run],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(sql_error)?;
        if !recorded {
            if members > 0 {
                let _ = w.conn.execute_batch("ROLLBACK");
                return Err(failure(
                    "the receipt run predates its acceptance record; its counts cannot be proven",
                ));
            }
            w.conn
                .execute(
                    "UPDATE collection_runs SET provenance_json=json_set(provenance_json,'$.first_accepted',json('[]')) WHERE run_id=?1",
                    params![run],
                )
                .map_err(sql_error)?;
        }
    }
    let (mut fresh, mut repeated) = (0u64, 0u64);
    for record in records {
        w.check()?;
        if let Record::Receipt { origin, .. } = record {
            let known: bool = w
                .conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM receipts WHERE origin_key=?1)",
                    [origin],
                    |r| r.get(0),
                )
                .map_err(sql_error)?;
            if known {
                repeated += 1;
            } else {
                fresh += 1;
            }
            store_with_reconciliation(&w.conn, record, run, observed, false)?;
            if !known && origin_label == "client_asserted" {
                w.conn
                    .execute(
                        "UPDATE collection_runs SET provenance_json=json_set(provenance_json,'$.first_accepted',json(json_insert(json_extract(provenance_json,'$.first_accepted'),'$[#]',?2))) WHERE run_id=?1 AND json_type(provenance_json,'$.first_accepted')='array'",
                        params![run, digest(origin)?],
                    )
                    .map_err(sql_error)?;
            }
            continue;
        }
        store_with_reconciliation(&w.conn, record, run, observed, false)?;
    }
    // Reconcile the coherent whole batch once per UUID. Per-record reconciliation
    // repeatedly scanned and updated the same receipt population before commit.
    let receipt_ids: Vec<_> = records
        .iter()
        .filter_map(|record| match record {
            Record::Receipt { origin, .. } => Some(digest(origin)),
            _ => None,
        })
        .collect::<Result<_, _>>()?;
    let scans:BTreeSet<String>=w.conn.prepare("SELECT DISTINCT scan_id FROM receipts WHERE receipt_id IN (SELECT value FROM json_each(?1)) AND scan_id IS NOT NULL").map_err(sql_error)?.query_map([encoded(&receipt_ids)?],|r|r.get(0)).map_err(sql_error)?.collect::<rusqlite::Result<_>>().map_err(sql_error)?;
    for scan in scans {
        w.check()?;
        reconcile_scan(&w.conn, &scan)?;
    }
    w.check()?;
    w.conn.execute_batch("COMMIT").map_err(sql_error)?;
    w.check()?;
    Ok((fresh, repeated))
}

/// The archive write budget of one service receipt batch (`receipts_ingest`).
pub const SERVICE_RECEIPT_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);

/// [`finish_received`] for a service receipt batch, under [`SERVICE_RECEIPT_BUDGET`].
pub fn finish_received_within(
    root: &Path,
    cloud: &Cloud,
    run: &str,
    state: &str,
    budget: std::time::Duration,
) -> Result<(), CliError> {
    finish_received_budget(root, cloud, run, state, budget)
}

pub fn finish_received(root: &Path, cloud: &Cloud, run: &str, state: &str) -> Result<(), CliError> {
    finish_received_budget(root, cloud, run, state, database::RETAIN_BUDGET)
}

fn finish_received_budget(
    root: &Path,
    cloud: &Cloud,
    run: &str,
    state: &str,
    budget: std::time::Duration,
) -> Result<(), CliError> {
    if !["complete", "partial", "interrupted"].contains(&state) {
        return Err(failure("invalid retention completion state"));
    }
    let w = Writer::existing_batch(root, cloud, budget, &scm::Session::new())?;
    w.conn.execute_batch("BEGIN IMMEDIATE").map_err(sql_error)?;
    w.conn.execute("UPDATE collection_runs SET ended_ms=?2,state=?3 WHERE run_id=?1 AND kind IN ('scan','bulk','resume')",params![run,database::now_ms()?,state]).map_err(sql_error)?;
    w.check()?;
    w.conn.execute_batch("COMMIT").map_err(sql_error)?;
    w.check()
}

/// Test support: one synthetic scan event recorded in its own completed sync run with a
/// controlled observation time, so service tests can place evidence before and after a
/// report's revision cut-offs.
#[doc(hidden)]
pub fn seed_observed_event_for_tests(
    root: &Path,
    cloud: &Cloud,
    event: Event,
    run_id: &str,
    observed_ms: i64,
) -> Result<(), CliError> {
    let session = scm::Session::new();
    let w = Writer::open(
        root,
        cloud,
        std::time::Duration::from_secs(10),
        true,
        &session,
    )?;
    w.conn.execute_batch("BEGIN IMMEDIATE").map_err(sql_error)?;
    w.conn
        .execute(
            "INSERT INTO collection_runs VALUES (?1,'sync',?2,?2,NULL,NULL,'complete','{}')",
            params![run_id, observed_ms],
        )
        .map_err(sql_error)?;
    store(
        &w.conn,
        &Record::Event(EventFact {
            event,
            identity_withheld: false,
            origin: "synthetic".into(),
            scanner_endpoint: None,
            scanner_reference: None,
            batch_report_id: None,
        }),
        run_id,
        observed_ms,
    )?;
    w.conn.execute_batch("COMMIT").map_err(sql_error)?;
    w.check()
}

#[cfg(test)]
#[path = "repository_tests.rs"]
mod tests;

/// Synthetic archive records for other modules' tests (report, query).
#[cfg(test)]
pub(crate) mod seed {
    use super::*;

    pub fn event(
        conn: &Connection,
        event: Event,
        run: &str,
        observed: i64,
    ) -> Result<(), CliError> {
        store(
            conn,
            &Record::Event(EventFact {
                event,
                identity_withheld: false,
                origin: "synthetic".into(),
                scanner_endpoint: None,
                scanner_reference: None,
                batch_report_id: None,
            }),
            run,
            observed,
        )
    }

    pub fn probe_receipt(conn: &Connection, event: Event, run: &str, observed: i64) {
        store(
            conn,
            &Record::Receipt {
                origin: "probe-receipt".into(),
                fact: EventFact {
                    event,
                    identity_withheld: false,
                    origin: "probe-receipt".into(),
                    scanner_endpoint: None,
                    scanner_reference: None,
                    batch_report_id: None,
                },
            },
            run,
            observed,
        )
        .unwrap();
    }

    pub fn profile(
        conn: &Connection,
        profile: Profile,
        run: &str,
        observed: i64,
    ) -> Result<(), CliError> {
        let key = profile.name.clone();
        store(
            conn,
            &Record::Profile {
                profile,
                key,
                linked: true,
            },
            run,
            observed,
        )
    }

    pub fn coverage(
        conn: &Connection,
        source: Source,
        (since, until): (i64, i64),
        run: &str,
        observed: i64,
    ) -> Result<(), CliError> {
        store(
            conn,
            &Record::Coverage {
                source,
                since,
                until,
                selected_apps: vec![],
                scope_supported: true,
                native: true,
            },
            run,
            observed,
        )
    }
}
