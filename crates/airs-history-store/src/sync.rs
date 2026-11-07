//! Explicit sync owns a network lease, but opens SQLite only for bounded local batches.
pub use super::work::Source;
pub(crate) use super::work::{Attempt, Descriptor, Outcome};
use super::{
    database::{self, Lease, Writer, sql_error},
    failure,
    identity::Cloud,
    repository,
    work::{self, Admission},
};
use airs_runtime_core::{
    error::{CliError, SecretSet},
    signal::{evidence::Evidence, privacy::PayloadSpans},
    tenant::{TenantContext, scanners::ScannerReference},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

type Reservations = (Vec<(Descriptor, Attempt)>, Vec<String>);
#[path = "sync_backlog.rs"]
mod backlog;

pub struct Sync {
    backlog_reserved: std::sync::atomic::AtomicU64,
    /// Selected examples served from the archive instead of a Reports call.
    cached_examples: std::sync::atomic::AtomicU64,
    backlog_example_ids: std::sync::atomic::AtomicU64,
    journal_failure: std::sync::Mutex<bool>,
    journal_admission_notes: std::sync::Mutex<std::collections::BTreeSet<&'static str>>,
    origins: std::sync::Mutex<BTreeMap<airs_runtime_core::signal::evidence::Identity, String>>,
    root: PathBuf,
    cloud: Cloud,
    lease: Lease,
    until: Instant,
    session: scm::Session,
    secrets: SecretSet,
    payload: PayloadSpans,
    pub run: String,
    pub target: (i64, i64),
    pub apps: Vec<String>,
    pub references: BTreeMap<String, ScannerReference>,
    pub budget: std::sync::Arc<airs_runtime_core::read_budget::ReadBudget>,
    observed_bindings:
        std::sync::Mutex<BTreeMap<airs_runtime_core::signal::evidence::Identity, ScannerReference>>,
    /// The service's in-run transport admission fence (contract §6, §10); unset for the
    /// CLI-equivalent driver.
    fence: std::sync::OnceLock<SendFence>,
}

/// The service's in-run transport admission fence, consulted by the journal before
/// every management and OAuth transport of the run, in the transaction that records
/// the transport `started` (so the decision and the durable record cannot diverge).
#[derive(Clone, Copy, Debug)]
pub struct SendFence {
    /// The service's minimum management hold after an API 429, in seconds: committed
    /// atomically with the 429 as the archive's management cooldown (a longer provider
    /// `Retry-After` is kept), so the per-transport cooldown check enforces it for every
    /// later send, in this run and every other.
    pub management_hold_floor_seconds: u64,
    /// Refuse every later transport of the run once the run's journal holds a
    /// definitive credential refusal (see [`run_definitively_denied`]).
    pub credential: bool,
}

/// Whether run `run`'s journal holds a definitive credential refusal at once (the
/// running-run subset of `run_summary::revision_denied`): a token endpoint 401/403 with
/// no later token; an API 403; or an API operation refused twice (its single fresh-token
/// replay refused too) with no later success. A single API 401 is not definitive: its
/// one accounted fresh-token replay must still be admitted.
pub fn run_definitively_denied(conn: &Connection, run: &str) -> Result<bool, CliError> {
    conn.query_row(
        &format!(
            "SELECT EXISTS(SELECT 1 FROM management_reads d WHERE d.run_id=?1 AND d.outcome='auth_denied' AND (
               ({token})
               OR (d.source!='oauth.token' AND d.status=403)
               OR (d.source!='oauth.token'
                 AND EXISTS (SELECT 1 FROM management_reads x WHERE x.run_id=d.run_id AND x.source=d.source
                   AND x.descriptor_json=d.descriptor_json AND x.read_id<>d.read_id AND x.outcome='auth_denied')
                 AND NOT EXISTS (SELECT 1 FROM management_reads r WHERE r.run_id=d.run_id AND r.source=d.source
                   AND r.descriptor_json=d.descriptor_json AND r.read_id<>d.read_id AND r.started_ms>=d.started_ms
                   AND r.outcome='returned'))))",
            token = crate::run_summary::TOKEN_TERMINAL
        ),
        [run],
        |r| r.get(0),
    )
    .map_err(sql_error)
}

pub fn retry(
    root: &Path,
    cloud: &Cloud,
    source: Source,
    session: &scm::Session,
) -> Result<serde_json::Value, CliError> {
    let until = Instant::now() + Duration::from_secs(120);
    let lease = Writer::existing(root, cloud, Duration::from_secs(120), session)?.into_lease()?;
    let mut total = 0u64;
    loop {
        lease.check()?;
        let mut w = Writer::existing_batch(
            root,
            cloud,
            until.saturating_duration_since(Instant::now()),
            session,
        )?;
        let tx = w.conn.transaction().map_err(sql_error)?;
        let count = work::reset(&tx, source, database::now_ms()?)?;
        tx.commit().map_err(sql_error)?;
        w.check()?;
        total += count;
        if count == 0 {
            break;
        }
    }
    Ok(
        json!({"source":source.name(),"reset_work":total,"remote_requests":0,"authorization_history":"preserved; ineligible work remains ineligible","cooldowns":"preserved"}),
    )
}

impl airs_runtime_core::read_budget::ReadJournal for Sync {
    fn begin(&self, operation: &str, query: &[(String, String)]) -> scm::Result<String> {
        let result = (|| {
            let id = uuid::Uuid::new_v4().to_string();
            let now = database::now_ms()?;
            let mut descriptor = json!({"operation":operation,"query":query,"scope":"bounded transport descriptor; no saved server cursor","actual_send":"unconfirmed until transport completion"});
            airs_runtime_core::signal::privacy::document(
                &mut descriptor,
                &self.secrets,
                &self.payload,
            );
            let operation = descriptor
                .get("operation")
                .and_then(serde_json::Value::as_str)
                .filter(|s| !s.is_empty() && s.len() <= 128)
                .unwrap_or("management");
            let fence = self.fence.get().copied();
            self.batch(|conn|{
                if fence.is_some_and(|f| f.credential) && run_definitively_denied(conn, &self.run)? {self.journal_note("A definitive credential refusal in this run; no further request admitted");return Err(failure("definitive credential refusal in this run"));}
                if work::cooling_service(conn,"management",now)? {self.journal_note("Management service cooldown or uncertain wall clock; no request admitted");return Err(failure("management service cooldown or uncertain wall clock"));}
                let backwards:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM management_reads WHERE started_ms>?1)",[now],|r|r.get(0)).map_err(sql_error)?;
                if backwards{self.journal_note("Wall clock moved backwards; management work deferred without a request");return Err(failure("wall clock moved backwards; management work deferred"));}
                conn.execute("INSERT INTO management_reads VALUES(?1,?2,?3,NULL,NULL,?4,NULL,NULL,0,NULL,'started',?5)",params![id,self.run,operation,now,descriptor.to_string()]).map_err(sql_error)?;
                Ok(())
            })?;
            Ok::<_, CliError>(id)
        })();
        result.map_err(|_| scm::Error::new(scm::ErrorKind::Limit))
    }
    fn end(
        &self,
        id: &str,
        status: Option<u16>,
        sent: bool,
        retry_after: Option<u64>,
    ) -> scm::Result<()> {
        let result = (|| {
            self.lease.check_identity()?;
            let session = scm::Session::new();
            let mut w =
                Writer::existing_batch(&self.root, &self.cloud, Duration::from_secs(5), &session)?;
            let tx = w.conn.transaction().map_err(sql_error)?;
            let now = database::now_ms()?;
            let outcome = if !sent {
                "not_sent"
            } else if matches!(status, Some(401 | 403)) {
                "auth_denied"
            } else if status == Some(429) {
                "rate_limited"
            } else if status.is_some_and(|s| (200..300).contains(&s)) {
                "returned"
            } else if self.session.is_cancelled() {
                "interrupted"
            } else {
                "failed"
            };
            let changed=tx.execute("UPDATE management_reads SET ended_ms=?2,status=?3,actual_sends=?4,outcome=?5 WHERE read_id=?1 AND outcome='started'",params![id,now,status,i64::from(sent),outcome]).map_err(sql_error)?;
            if changed != 1 {
                return Err(failure("management read was already finished"));
            }
            if status == Some(429) {
                // The service's minimum hold follows an API 429 (not a token-endpoint
                // 429, which keeps the provider's duration), in the same transaction.
                let floor = match self.fence.get() {
                    Some(f) if f.management_hold_floor_seconds > 0 => {
                        let source: String = tx
                            .query_row(
                                "SELECT source FROM management_reads WHERE read_id=?1",
                                [id],
                                |r| r.get(0),
                            )
                            .map_err(sql_error)?;
                        (source != "oauth.token").then_some(f.management_hold_floor_seconds)
                    }
                    _ => None,
                };
                let seconds = match floor {
                    Some(floor) => Some(retry_after.filter(|s| *s > 0).unwrap_or(900).max(floor)),
                    None => retry_after,
                };
                work::cooldown_service(&tx, "management", now, seconds)?;
            }
            tx.commit().map_err(sql_error)?;
            w.check()?;
            Ok::<_, CliError>(())
        })();
        result.map_err(|_| {
            *self
                .journal_failure
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
            scm::Error::new(scm::ErrorKind::Limit)
        })
    }
}
impl Sync {
    fn journal_note(&self, note: &'static str) {
        self.journal_admission_notes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(note);
    }
    fn journal_notes(&self) -> serde_json::Value {
        json!(
            *self
                .journal_admission_notes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        )
    }
    pub fn sessions_discovered(
        &self,
        keys: &[(String, String, String)],
        window: &airs_runtime_core::signal::evidence::Window,
    ) -> Result<(), CliError> {
        use sha2::{Digest, Sha256};
        let mut records = vec![];
        for key in keys {
            let raw = json!({"application_id":key.0,"application_name":key.1,"session_id":key.2});
            let mut safe = raw.clone();
            airs_runtime_core::signal::privacy::document(&mut safe, &self.secrets, &self.payload);
            if raw != safe
                || [&key.0, &key.1, &key.2].iter().any(|s| {
                    s.is_empty()
                        || [
                            airs_runtime_core::signal::privacy::WITHHELD,
                            airs_runtime_core::error::WITHHELD,
                            "[REDACTED]",
                        ]
                        .contains(&s.as_str())
                })
            {
                continue;
            }
            let descriptor = safe.to_string();
            let id = format!("{:x}", Sha256::digest(format!("session/{descriptor}")));
            if descriptor.len() + 1024 > 256 * 1024 {
                return Err(failure("session work exceeds normalized batch bound"));
            }
            records.push((id, descriptor));
        }
        let mut next = 0;
        while next < records.len() {
            let mut end = next;
            let mut bytes = 0;
            while end < records.len()
                && end - next < 200
                && bytes + records[end].1.len() + 1024 <= 256 * 1024
            {
                bytes += records[end].1.len() + 1024;
                end += 1;
            }
            self.batch(|conn|{
                for(id,descriptor)in &records[next..end]{
                    conn.execute("INSERT INTO sync_work VALUES(?1,'session',?2,'pending',NULL,0,0,NULL,NULL,NULL,?3) ON CONFLICT(work_id) DO UPDATE SET state='pending',provenance_json=CASE WHEN state='complete' THEN excluded.provenance_json ELSE provenance_json END",params![id,descriptor,json!({"first_query_since_ms":window.start_ms,"first_query_until_ms":window.end_ms,"first_run":self.run,"scope":"head traversal; no server cursor; not a provider watermark"}).to_string()]).map_err(sql_error)?;
                }
                Ok(())
            })?;
            next = end;
        }
        Ok(())
    }
    pub fn session_finished(
        &self,
        key: (&str, &str, &str),
        window: &airs_runtime_core::signal::evidence::Window,
        complete: bool,
    ) -> Result<(), CliError> {
        use sha2::{Digest, Sha256};
        let descriptor =
            json!({"application_id":key.0,"application_name":key.1,"session_id":key.2}).to_string();
        let id = format!("{:x}", Sha256::digest(format!("session/{descriptor}")));
        self.batch(|conn|{
            let first:Option<i64>=conn.query_row("SELECT json_extract(provenance_json,'$.first_query_since_ms') FROM sync_work WHERE work_id=?1 AND kind='session'",[&id],|r|r.get(0)).optional().map_err(sql_error)?.flatten();
            let Some(first)=first else{return Ok(())};
            let state=if complete&&window.start_ms<=first{"complete"}else if complete{"falling_behind"}else{"pending"};
            conn.execute("UPDATE sync_work SET state=?2,reason=?3 WHERE work_id=?1",params![id,state,if state=="falling_behind"{"recent head traversal cannot establish completion of the earlier requested tail"}else{"current head traversal state; boundary and late-arrival uncertainty retained"}]).map_err(sql_error)?;
            if complete{conn.execute("INSERT INTO work_transitions VALUES(?1,?2,?3,?4,0,NULL,NULL,?5,?6)",params![uuid::Uuid::new_v4().to_string(),id,database::now_ms()?,if state=="complete"{"completed"}else{"falling_behind"},"session head traversal; no scanner attempt or epoch",json!({"run_id":self.run,"actual_rolling_query":window,"first_query_since_ms":first}).to_string()]).map_err(sql_error)?;}
            Ok(())
        })
    }
    fn final_batch<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, CliError>,
    ) -> Result<T, CliError> {
        self.lease.check_identity()?;
        let session = scm::Session::new();
        let mut w =
            Writer::existing_batch(&self.root, &self.cloud, Duration::from_secs(5), &session)?;
        let tx = w.conn.transaction().map_err(sql_error)?;
        let result = f(&tx)?;
        tx.commit().map_err(sql_error)?;
        w.check()?;
        Ok(result)
    }
    pub fn abort(&self) -> Result<(), CliError> {
        self.final_batch(|conn|{conn.execute("UPDATE collection_runs SET ended_ms=?2,state=?3,provenance_json=json_set(provenance_json,'$.management_admission_notes',json(?4)) WHERE run_id=?1 AND state='running'",params![self.run,database::now_ms()?,if self.session.is_cancelled(){"interrupted"}else{"failed"},self.journal_notes().to_string()]).map_err(sql_error)?;Ok(())})
    }
    fn observed_cooldown(
        &self,
        source: Source,
        response: (Option<u16>, Option<u64>),
    ) -> Result<(), CliError> {
        if response.0 == Some(429) {
            self.final_batch(|conn| work::cooldown(conn, source, database::now_ms()?, response.1))?;
        }
        Ok(())
    }
    pub fn journal(self: &std::sync::Arc<Self>) {
        let journal: std::sync::Arc<dyn airs_runtime_core::read_budget::ReadJournal> = self.clone();
        self.budget.journal(std::sync::Arc::downgrade(&journal));
    }
    fn referenced_sources(e: &mut Evidence) -> Result<(), CliError> {
        use airs_runtime_core::signal::evidence::{Source, Status};
        let names: std::collections::BTreeSet<_> = e
            .events
            .iter()
            .flat_map(|e| e.sources.iter().cloned())
            .collect();
        for name in names {
            if !e.sources.iter().any(|s| s.name == name) {
                let now = database::now_ms()?;
                e.sources.push(Source {
                    name,
                    status: Status::Partial,
                    required: false,
                    records: 0,
                    pages: 0,
                    unit: "retained or received observation references".into(),
                    population: "referenced observations; no fresh traversal implied".into(),
                    started_ms: now,
                    ended_ms: now,
                    notes: vec![
                        "Referenced facts retained; current traversal completeness is separate"
                            .into(),
                    ],
                });
            }
        }
        Ok(())
    }
    fn fragment(
        &self,
        evidence: &Evidence,
        events: Vec<airs_runtime_core::signal::evidence::Event>,
    ) -> Result<std::collections::BTreeSet<airs_runtime_core::signal::evidence::Identity>, CliError>
    {
        let mut fragment = evidence.clone();
        fragment.events = events;
        fragment.profiles.clear();
        fragment.applications.clear();
        fragment.metrics.clear();
        fragment
            .sampling
            .selected_report_ids
            .retain(|id| fragment.events.iter().any(|e| e.report_ids.contains(id)));
        Self::referenced_sources(&mut fragment)?;
        self.checkpoint_inner(&fragment)
    }
    /// App queues rotate after each five-identity transport. A busy app cannot
    /// consume all enrichment capacity before another selected app is considered.
    pub async fn enrich(
        &self,
        inv: &airs_runtime_core::signal::sink::CollectContext<'_>,
        tenant: &TenantContext,
        e: &mut Evidence,
        old_reports: Vec<backlog::Item>,
    ) -> Result<(), CliError> {
        use airs_runtime_core::signal::{
            collect,
            evidence::{Source as EvidenceSource, Status},
        };
        use std::collections::VecDeque;
        let rolling = e.events.len();
        e.events.retain(|event| {
            event
                .timestamp_ms
                .is_none_or(|t| t >= self.target.0 && t < self.target.1)
        });
        let unknown = e.events.iter().filter(|e| e.timestamp_ms.is_none()).count();
        let now = database::now_ms()?;
        e.sources.push(EvidenceSource{name:"archive-range".into(),status:if unknown>0{Status::Partial}else{Status::Complete},required:true,records:(e.events.len()-unknown) as u64,pages:0,unit:"timestamp-eligible requested-cohort events".into(),population:"local half-open absolute range; rolling observations outside excluded before scanner admission".into(),started_ms:now,ended_ms:now,notes:vec![format!("{} rolling observations outside requested interval excluded; {unknown} observations have unknown timestamps and remain ineligible for window totals",rolling-e.events.len())]});
        let mut queues: BTreeMap<String, VecDeque<usize>> = BTreeMap::new();
        for (index, event) in e.events.iter_mut().enumerate() {
            if let Ok(id) = uuid::Uuid::parse_str(&event.identity.scan_id) {
                event.identity.scan_id = id.to_string();
            }
            queues
                .entry(event.identity.app_id.clone())
                .or_default()
                .push_back(index);
        }
        let mut active: VecDeque<_> = queues.keys().cloned().collect();
        let mut scanners = BTreeMap::new();
        for app in &active {
            if let Some((_, binding)) = tenant.config.scanner_for_application(app)
                && self.references.contains_key(app)
                && let Ok(scanner) = airs_runtime_core::clients::build_scanner_bounded(
                    &binding.overlay(&tenant.config),
                    inv.session,
                    None,
                    Some(&self.budget),
                )
            {
                scanners.insert(app.clone(), scanner);
            }
        }
        let now = database::now_ms()?;
        let mut source = EvidenceSource {
            name: "scan-results".into(),
            status: Status::Complete,
            required: true,
            records: 0,
            pages: 0,
            unit: "correlated scan result rows".into(),
            population: "selected application scope; retained and newly collected observations"
                .into(),
            started_ms: now,
            ended_ms: now,
            notes: vec![],
        };
        while let Some(app) = active.pop_front() {
            let queue = queues.get_mut(&app).expect("scheduled app exists");
            let indices: Vec<_> = (0..5).filter_map(|_| queue.pop_front()).collect();
            if !queue.is_empty() {
                active.push_back(app.clone());
            }
            let mut descriptors = vec![];
            for index in indices {
                let event = &e.events[index];
                if !airs_runtime_core::scan_logs::is_scan_id(&event.identity.scan_id)
                    || event.identity.sub_request.is_none()
                {
                    source.status = Status::Partial;
                    source
                        .notes
                        .push("Opaque or incomplete scan identity remains unresolved".into());
                    continue;
                }
                let cached = self.batch(|conn| repository::cached_event(conn, &event.identity))?;
                if let Some(cached) = cached.filter(|v| {
                    v.sources.iter().any(|s| s == "scan-results")
                        && ["benign", "malicious"].contains(&v.verdict.as_str())
                        && ["allow", "block"].contains(&v.action.as_str())
                }) {
                    e.events[index] = cached;
                    source.records += 1;
                    continue;
                }
                descriptors.push(Descriptor {
                    identity: e.events[index].identity.clone(),
                    report_id: None,
                });
            }
            if descriptors.is_empty() {
                continue;
            }
            let mut contexts =
                self.batch(|conn| repository::retained_contexts(conn, &descriptors))?;
            contexts.extend(e.events.iter().cloned());
            let (admitted, notes) = self.reserve(&descriptors, Source::Results, &app)?;
            if !notes.is_empty() {
                source.status = Status::Partial;
                source.notes.extend(notes);
            }
            if admitted.is_empty() {
                continue;
            }
            let Some((scanner, status)) = scanners.get(&app) else {
                self.outcomes(
                    &admitted
                        .iter()
                        .map(|(_, a)| (a.clone(), Outcome::NotSent))
                        .collect::<Vec<_>>(),
                    None,
                    Source::Results,
                    None,
                )?;
                source.status = Status::Partial;
                source.notes.push(
                    "No usable application-bound scanner; management observations retained".into(),
                );
                continue;
            };
            let mut events: Vec<_> = admitted
                .iter()
                .map(|(d, _)| {
                    e.events
                        .iter()
                        .find(|e| e.identity == d.identity)
                        .expect("admitted event exists")
                        .clone()
                })
                .collect();
            let before = status.attempts("airs.scan.results");
            let observed = collect::enrich_in_context(
                scanner,
                status,
                &self.budget,
                &mut events,
                inv.secrets,
                Some(&contexts),
            )
            .await;
            let attributed = observed.attributed;
            let observed = observed.source;
            let sent = status.attempts("airs.scan.results") > before;
            let finalized = self.finish_transport_only(
                &admitted,
                sent,
                status.response("airs.scan.results"),
                Source::Results,
            )?;
            if sent {
                self.observed_cooldown(Source::Results, status.response("airs.scan.results"))?;
            }
            if sent && let Some(reference) = self.references.get(&app) {
                let mut bindings = self
                    .observed_bindings
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                for event in &events {
                    if attributed.contains(&event.identity) {
                        bindings.insert(event.identity.clone(), reference.clone());
                    }
                }
            }

            source.records += observed.records;
            source.pages += observed.pages;
            source.ended_ms = database::now_ms()?;
            if observed.status != Status::Complete {
                source.status = Status::Partial;
            }
            source.notes.extend(observed.notes);
            for event in &events {
                if let Some(old) = e.events.iter_mut().find(|v| v.identity == event.identity) {
                    *old = event.clone();
                }
            }
            // Persist facts first. A crash before completion keeps conservative
            // started attempts; a later run can use the already retained facts.
            let retained = self.fragment(e, events.clone())?;
            let outcomes = admitted
                .into_iter()
                .filter(|_| !finalized)
                .map(|(d, a)| {
                    let resolved = attributed.contains(&d.identity)
                        && retained.contains(&d.identity)
                        && events.iter().any(|e| {
                            e.identity == d.identity
                                && e.sources.iter().any(|s| s == "scan-results")
                                && ["benign", "malicious"].contains(&e.verdict.as_str())
                                && ["allow", "block"].contains(&e.action.as_str())
                                && e.timestamp_ms
                                    .is_none_or(|t| t >= self.target.0 && t < self.target.1)
                        });
                    let outcome = if !sent {
                        Outcome::NotSent
                    } else if matches!(status.response("airs.scan.results").0, Some(401 | 403)) {
                        Outcome::AuthDenied
                    } else if status.response("airs.scan.results").0 == Some(429) {
                        Outcome::RateLimited
                    } else if resolved {
                        Outcome::Returned
                    } else if inv.session.is_cancelled() {
                        Outcome::Interrupted
                    } else if status
                        .response("airs.scan.results")
                        .0
                        .is_none_or(|s| s >= 500)
                    {
                        Outcome::Failed
                    } else if events.iter().any(|e| {
                        e.identity == d.identity
                            && e.notes
                                .iter()
                                .any(|n| n == "Incomplete scan outcome; never interpreted as allow")
                    }) {
                        Outcome::Pending
                    } else {
                        Outcome::Missing
                    };
                    (a, outcome)
                })
                .collect::<Vec<_>>();
            self.outcomes(
                &outcomes,
                if sent {
                    status.response("airs.scan.results").0
                } else {
                    None
                },
                Source::Results,
                status.response("airs.scan.results").1,
            )?;
        }
        source.notes.sort();
        source.notes.dedup();
        source.ended_ms = database::now_ms()?;
        if let Some(prior) = e.sources.iter_mut().find(|s| s.name == "scan-results") {
            *prior = source;
        } else {
            e.sources.push(source);
        }
        Self::referenced_sources(e)?;
        self.checkpoint(e)?;
        e.events.retain(|event| {
            event
                .timestamp_ms
                .is_none_or(|t| t >= self.target.0 && t < self.target.1)
        });
        // Selection is deterministic and optional. Reports keep their own durable
        // work and share the invocation's hard two-send/ten-ID transport budget.
        if e.budgets.max_reports > 0 {
            let selected_old = self.select_examples(e, old_reports)?;
            self.revisit(inv, tenant, e, selected_old).await?;
            let selected = e.sampling.selected_report_ids.clone();
            let mut owners: BTreeMap<String, Vec<usize>> = BTreeMap::new();
            for (index, event) in e.events.iter().enumerate() {
                for id in &event.report_ids {
                    owners.entry(id.clone()).or_default().push(index);
                }
            }
            let mut groups: BTreeMap<String, Vec<Descriptor>> = BTreeMap::new();
            let mut order = vec![];
            for id in selected {
                let Some(indices) = owners.get(&id).filter(|indices| indices.len() == 1) else {
                    continue;
                };
                let event = &e.events[indices[0]];
                if self.batch(|conn| repository::cached_example(conn, &event.identity, &id))? {
                    self.cached_examples
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    continue;
                }
                let app = event.identity.app_id.clone();
                if !order.contains(&app) {
                    order.push(app.clone());
                }
                groups.entry(app).or_default().push(Descriptor {
                    identity: event.identity.clone(),
                    report_id: Some(id),
                });
            }
            for app in order {
                for descriptors in groups[&app].chunks(5) {
                    let (admitted, notes) = self.reserve(descriptors, Source::Reports, &app)?;
                    if !notes.is_empty() {
                        e.sampling.uncovered_groups.extend(notes);
                    }
                    if admitted.is_empty() {
                        continue;
                    }
                    let Some((scanner, status)) = scanners.get(&app) else {
                        self.outcomes(
                            &admitted
                                .iter()
                                .map(|(_, a)| (a.clone(), Outcome::NotSent))
                                .collect::<Vec<_>>(),
                            None,
                            Source::Reports,
                            None,
                        )?;
                        continue;
                    };
                    let ids: Vec<_> = admitted
                        .iter()
                        .filter_map(|(d, _)| d.report_id.clone())
                        .collect();
                    let mut events: Vec<_> = e
                        .events
                        .iter()
                        .filter(|e| admitted.iter().any(|(d, _)| d.identity == e.identity))
                        .cloned()
                        .collect();
                    let before = status.attempts("airs.scan.reports");
                    let observed = collect::details_attributed(
                        scanner,
                        status,
                        &self.budget,
                        &mut events,
                        &ids,
                        inv.secrets,
                        inv.payload_spans,
                    )
                    .await;
                    let attributed = observed.attributed;
                    let observed = observed.source;
                    let sent = status.attempts("airs.scan.reports") > before;
                    let response = status.response("airs.scan.reports");
                    let finalized =
                        self.finish_transport_only(&admitted, sent, response, Source::Reports)?;
                    if sent && let Some(reference) = self.references.get(&app) {
                        let mut bindings = self
                            .observed_bindings
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        for event in &events {
                            if attributed.contains(&event.identity) {
                                bindings.insert(event.identity.clone(), reference.clone());
                            }
                        }
                    }

                    if sent {
                        self.observed_cooldown(Source::Reports, response)?;
                    }
                    for event in &events {
                        if let Some(old) =
                            e.events.iter_mut().find(|e| e.identity == event.identity)
                        {
                            *old = event.clone();
                        }
                    }
                    if let Some(prior) = e.sources.iter_mut().find(|s| s.name == "scan-reports") {
                        prior.records += observed.records;
                        prior.pages += observed.pages;
                        prior.ended_ms = observed.ended_ms;
                        prior.notes.extend(observed.notes);
                        if observed.status != Status::Complete {
                            prior.status = Status::Partial;
                        }
                    } else {
                        e.sources.push(observed);
                    }
                    let retained = self.fragment(e, events.clone())?;
                    let outcomes=admitted.into_iter().filter(|_| !finalized).map(|(d,a)|{
                        let usable=attributed.contains(&d.identity)&&retained.contains(&d.identity)&&events.iter().any(|e|e.identity==d.identity&&e.detections.iter().filter(|det|Some(det.report_id.as_str())==d.report_id.as_deref()).any(|det|det.snippets.iter().any(|s|matches!(s.state,airs_runtime_core::signal::evidence::SnippetState::Present|airs_runtime_core::signal::evidence::SnippetState::Empty|airs_runtime_core::signal::evidence::SnippetState::Withheld))));
                        let outcome=if !sent{Outcome::NotSent}else if matches!(response.0,Some(401|403)){Outcome::AuthDenied}else if response.0==Some(429){Outcome::RateLimited}else if usable{Outcome::Returned}else{Outcome::Rejected};
                        (a,outcome)
                    }).collect::<Vec<_>>();
                    self.outcomes(
                        &outcomes,
                        if sent { response.0 } else { None },
                        Source::Reports,
                        response.1,
                    )?;
                }
            }
        }
        e.collected_ms = database::now_ms()?;
        e.budgets.attempts = self.budget.attempts();
        e.seal_native()?;
        Ok(())
    }
    pub fn start(
        root: &Path,
        tenant: &TenantContext,
        target: (i64, i64),
        controls: (Duration, bool, Vec<String>, Option<String>, Option<String>),
        protection: (&SecretSet, &PayloadSpans),
        session: &scm::Session,
        network: std::sync::Arc<airs_runtime_core::read_budget::ReadBudget>,
    ) -> Result<Self, CliError> {
        let (budget, backfill, apps, run_id, credential_revision) = controls;
        // The service binds the run to the credential revision it admitted (contract
        // §6.1): the archive journal is then the authority for that revision's denial.
        if credential_revision
            .as_deref()
            .is_some_and(|r| !airs_runtime_core::dto::is_uuid_v4(r))
        {
            return Err(failure("invalid credential revision"));
        }
        let mut safe_apps = json!(apps);
        airs_runtime_core::signal::privacy::document(&mut safe_apps, protection.0, protection.1);
        if safe_apps != json!(apps)
            || apps.iter().any(|s| {
                [
                    airs_runtime_core::signal::privacy::WITHHELD,
                    airs_runtime_core::error::WITHHELD,
                    "[REDACTED]",
                ]
                .contains(&s.as_str())
            })
        {
            return Err(failure(
                "application scope cannot be archived intact after privacy projection",
            ));
        }

        let cloud = Cloud::from_tenant(tenant)?;
        cloud.check_public(protection.0)?;
        let references = tenant
            .config
            .scanners()
            .into_iter()
            .flat_map(|m| m.iter())
            .map(|(name, binding)| (binding.application_id.clone(), binding.reference(name)))
            .filter(|(_, r)| {
                r.valid()
                    && [&r.name, &r.application_id, &r.revision, &r.endpoint]
                        .iter()
                        .all(|v| !protection.0.carried_by(v))
            })
            .collect();
        // A caller-chosen run ID (the service checkpoints it before any archive write,
        // crash point C4) must be a UUID; otherwise a fresh one is minted.
        let run = match run_id {
            Some(id) if uuid::Uuid::parse_str(&id).is_ok() => id,
            Some(_) => return Err(failure("invalid collection run ID")),
            None => uuid::Uuid::new_v4().to_string(),
        };
        let now = database::now_ms()?;
        let mut w = Writer::sync(root, &cloud, budget, session)?;
        let tx = w.conn.transaction().map_err(sql_error)?;
        let bootstrap: Option<i64> = tx
            .query_row(
                "SELECT bootstrap_ms FROM archive_meta WHERE singleton=1",
                [],
                |r| r.get(0),
            )
            .map_err(sql_error)?;
        if !backfill && bootstrap.is_some_and(|b| target.0 < b) {
            return Err(failure(
                "requested interval predates archive bootstrap; use explicit --backfill",
            ));
        }

        let previous:Option<i64>=tx.query_row("SELECT max(target_until_ms) FROM collection_runs WHERE kind IN ('sync','refresh') AND ended_ms IS NOT NULL AND json_extract(provenance_json,'$.selected_app_ids')=json(?1)",[json!(apps).to_string()],|r|r.get(0)).map_err(sql_error)?;
        tx.execute("INSERT INTO collection_runs VALUES(?1,'sync',?2,NULL,?3,?4,'running',?5)",params![run,now,target.0,target.1,{let mut p=json!({"origin":"native_sync","absolute_bounds":"local filter only","coverage":"traversed as of collection, not provider watermark","selected_app_ids":apps});if let Some(r)=&credential_revision{p["credential_revision"]=json!(r);p["fact_counts"]=json!({"new":0,"duplicate":0});}p.to_string()}]).map_err(sql_error)?;
        if let Some(previous) = previous.filter(|previous| *previous < target.0) {
            tx.execute("INSERT INTO coverage_segments VALUES(?1,?2,NULL,NULL,'offline-gap',?3,?4,?5,'falling_behind',?6)",params![uuid::Uuid::new_v4().to_string(),run,previous,target.0,now,json!({"scope_kind":if apps.is_empty(){"cloud"}else{"selection"},"scope_identity_supported":true,"selected_app_ids":apps,"coverage_semantics":"unobserved gap between collection attempts; no silent historical backfill or claim of absence"}).to_string()]).map_err(sql_error)?;
        }
        tx.execute(
            "UPDATE archive_meta SET bootstrap_ms=coalesce(bootstrap_ms,?1) WHERE singleton=1",
            [target.0],
        )
        .map_err(sql_error)?;
        tx.commit().map_err(sql_error)?;
        let until = w.until;
        let lease = w.into_lease()?;
        Ok(Self {
            root: root.into(),
            cloud,
            lease,
            until,
            session: session.clone(),
            secrets: protection.0.clone(),
            payload: protection.1.clone(),
            run,
            target,
            apps,
            references,
            budget: network,
            observed_bindings: Default::default(),
            fence: std::sync::OnceLock::new(),
            journal_failure: Default::default(),
            journal_admission_notes: Default::default(),
            origins: Default::default(),
            backlog_reserved: Default::default(),
            cached_examples: Default::default(),
            backlog_example_ids: Default::default(),
        })
    }
    fn batch<T>(&self, f: impl FnOnce(&Connection) -> Result<T, CliError>) -> Result<T, CliError> {
        if *self
            .journal_failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            return Err(failure(
                "management transport journal failed; committed progress remains incomplete",
            ));
        }
        self.lease.check()?;
        let remaining = self.until.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(failure(
                "sync deadline exceeded; committed progress remains incomplete",
            ));
        }
        let mut w = Writer::existing_batch(
            &self.root,
            &self.cloud,
            remaining.min(Duration::from_secs(5)),
            &self.session,
        )?;
        let tx = w.conn.transaction().map_err(sql_error)?;
        let value = f(&tx)?;
        tx.commit().map_err(sql_error)?;
        w.check()?;
        Ok(value)
    }
    /// Normalization happens before the exclusive gate. Every committed snapshot
    /// remains attributable to the running collection after interruption.
    pub fn checkpoint(&self, evidence: &Evidence) -> Result<(), CliError> {
        self.checkpoint_inner(evidence).map(|_| ())
    }
    fn checkpoint_inner(
        &self,
        evidence: &Evidence,
    ) -> Result<std::collections::BTreeSet<airs_runtime_core::signal::evidence::Identity>, CliError>
    {
        let mut evidence = evidence.clone();
        evidence.collection_id = self.run.clone();
        evidence.collected_ms = database::now_ms()?;
        evidence.seal_native()?;
        let mut origins = self
            .origins
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for event in &evidence.events {
            origins.entry(event.identity.clone()).or_insert_with(|| {
                format!("native/{}/observation/{}", self.run, uuid::Uuid::new_v4())
            });
        }
        let records = repository::native_records(
            &evidence,
            &self.secrets,
            &self.payload,
            &self
                .observed_bindings
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            self.target,
            &origins,
        )?;
        drop(origins);
        self.batch(|conn|{
            conn.execute("UPDATE collection_runs SET provenance_json=json_insert(provenance_json,'$.actual_query',json(?2),'$.rolling_window_anchor',json(?3)) WHERE run_id=?1",params![self.run,json!({"time_interval":evidence.window.interval,"time_unit":evidence.window.unit,"absolute_selector":false,"semantics":"relative selector; each management read has its own traversal timestamp"}).to_string(),json!(evidence.window).to_string()]).map_err(sql_error)?;Ok(())
        })?;
        let mut next = 0;
        while next < records.len() {
            let end = records.batch_end(next);
            self.batch(|conn| records.store(conn, next, end, &self.run, evidence.collected_ms))?;
            next = end;
        }
        Ok(records.identities())
    }
    pub fn reserve(
        &self,
        descriptors: &[Descriptor],
        source: Source,
        application: &str,
    ) -> Result<Reservations, CliError> {
        self.reserve_scoped(descriptors, source, application, false)
    }
    fn reserve_scoped(
        &self,
        descriptors: &[Descriptor],
        source: Source,
        application: &str,
        revisit: bool,
    ) -> Result<Reservations, CliError> {
        if descriptors.len() > 5 {
            return Err(failure("scanner admission batch exceeds five identities"));
        }
        let reference = self.references.get(application);
        let reference_bytes = serde_json::to_vec(&reference)
            .map_err(|_| failure("invalid public binding"))?
            .len();
        let mut safe = vec![];
        let mut notes = vec![];
        for descriptor in descriptors {
            if descriptor.id(source).is_err() {
                notes.push("Incomplete or withheld work identity; no credential tried".into());
                continue;
            }
            let raw =
                serde_json::to_value(descriptor).map_err(|_| failure("invalid work descriptor"))?;
            let mut shown = raw.clone();
            airs_runtime_core::signal::privacy::document(&mut shown, &self.secrets, &self.payload);
            if shown != raw {
                notes.push(
                    "Work identity withheld by privacy projection; no credential tried".into(),
                );
                continue;
            }
            let bytes = raw
                .to_string()
                .len()
                .saturating_add(reference_bytes.saturating_mul(2))
                .saturating_add(2048);
            if bytes > 256 * 1024 {
                return Err(failure(
                    "normalized work admission exceeds the bounded transaction view; no scanner request sent",
                ));
            }
            safe.push((descriptor, bytes));
        }
        let transport = uuid::Uuid::new_v4().to_string();
        let mut next = 0;
        let mut admitted: Vec<(Descriptor, Attempt)> = vec![];
        while next < safe.len() {
            let mut end = next;
            let mut bytes = 0;
            while end < safe.len() && bytes + safe[end].1 <= 256 * 1024 {
                bytes += safe[end].1;
                end += 1;
            }
            let result=self.batch(|conn|{
                let mut rows=vec![];let mut skipped=vec![];
                for(descriptor,_)in &safe[next..end]{
                    if source==Source::Reports&&!repository::report_ownership(conn,descriptor)?{skipped.push("Report ownership is missing or ambiguous in retained history; no credential tried".into());continue;}
                    match work::reserve(conn,descriptor,source,reference,&self.run,&transport,database::now_ms()?)?{
                        Admission::Due(attempt)=>{
                            if revisit{conn.execute("UPDATE work_attempts SET provenance_json=json_set(provenance_json,'$.scope','individual_work_revisit','$.requested_run',?2) WHERE attempt_id=?1 AND outcome='started'",params![attempt.id,self.run]).map_err(sql_error)?;}
                            rows.push(((*descriptor).clone(),attempt));
                        }
                        Admission::Skip(reason)=>skipped.push(reason.into()),
                    }
                }
                Ok((rows,skipped))
            });
            match result {
                Ok((rows, skipped)) => {
                    admitted.extend(rows);
                    notes.extend(skipped);
                }
                Err(error) => {
                    for (_, attempt) in &admitted {
                        if self
                            .final_batch(|conn| {
                                work::finish(
                                    conn,
                                    attempt,
                                    Outcome::NotSent,
                                    None,
                                    database::now_ms()?,
                                )
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    return Err(error);
                }
            }
            next = end;
        }
        Ok((admitted, notes))
    }
    /// Persist observations requiring no retained response facts before any
    /// cancellation-sensitive snapshot. A proven refusal to send releases its
    /// ordinal; observed authorization/rate failures survive cancellation.
    fn finish_transport_only(
        &self,
        admitted: &[(Descriptor, Attempt)],
        sent: bool,
        response: (Option<u16>, Option<u64>),
        source: Source,
    ) -> Result<bool, CliError> {
        let outcome = if !sent {
            Some(Outcome::NotSent)
        } else if matches!(response.0, Some(401 | 403)) {
            Some(Outcome::AuthDenied)
        } else if response.0 == Some(429) {
            Some(Outcome::RateLimited)
        } else {
            None
        };
        let Some(outcome) = outcome else {
            return Ok(false);
        };
        self.outcomes(
            &admitted
                .iter()
                .map(|(_, a)| (a.clone(), outcome))
                .collect::<Vec<_>>(),
            if sent { response.0 } else { None },
            source,
            if sent { response.1 } else { None },
        )?;
        Ok(true)
    }
    pub fn outcomes(
        &self,
        attempts: &[(Attempt, Outcome)],
        status: Option<u16>,
        source: Source,
        retry_after: Option<u64>,
    ) -> Result<(), CliError> {
        let mut next = 0;
        while next < attempts.len() {
            let mut end = next;
            let mut bytes = 0;
            while end < attempts.len()
                && end - next < 200
                && bytes + attempts[end].0.bytes <= 256 * 1024
            {
                bytes += attempts[end].0.bytes;
                end += 1;
            }
            if end == next {
                return Err(failure(
                    "attempt completion exceeds the bounded transaction view",
                ));
            }
            self.final_batch(|conn| {
                for (attempt, outcome) in &attempts[next..end] {
                    work::finish(conn, attempt, *outcome, status, database::now_ms()?)?;
                }
                Ok(())
            })?;
            next = end;
        }
        if status == Some(429) {
            self.observed_cooldown(source, (status, retry_after))?;
        }
        Ok(())
    }
    fn progress(&self, conn: &Connection) -> Result<serde_json::Value, CliError> {
        let mut q=conn.prepare("SELECT kind,state,count(*) FROM sync_work WHERE json_array_length(?1)=0 OR coalesce(json_extract(descriptor_json,'$.identity.app_id'),json_extract(descriptor_json,'$.application_id')) IN (SELECT value FROM json_each(?1)) GROUP BY kind,state ORDER BY kind,state").map_err(sql_error)?;
        let rows = q
            .query_map([json!(self.apps).to_string()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(sql_error)?;
        let mut work = vec![];
        for row in rows {
            let (kind, state, count) = row.map_err(sql_error)?;
            work.push(json!({"source":kind,"state":state,"work_descriptors":count}));
        }
        let partial:i64=conn.query_row("SELECT count(DISTINCT f.receipt_id) FROM receipt_runs r JOIN receipt_facts f ON f.fact_id=r.fact_id WHERE r.run_id=?1",[&self.run],|r|r.get(0)).map_err(sql_error)?;
        let full:i64=conn.query_row("SELECT count(DISTINCT f.event_id) FROM fact_runs r JOIN event_facts f ON f.fact_id=r.fact_id WHERE r.run_id=?1",[&self.run],|r|r.get(0)).map_err(sql_error)?;
        let uncertain: i64 = conn
            .query_row(
                "SELECT count(*) FROM management_reads WHERE run_id=?1 AND outcome='started'",
                [&self.run],
                |r| r.get(0),
            )
            .map_err(sql_error)?;
        let revisit:i64=conn.query_row("SELECT count(*) FROM work_attempts WHERE run_id=?1 AND json_extract(provenance_json,'$.scope')='individual_work_revisit' AND outcome!='not_sent'",[&self.run],|r|r.get(0)).map_err(sql_error)?;
        let revisit_incomplete:i64=conn.query_row("SELECT count(*) FROM work_attempts WHERE run_id=?1 AND source='results' AND json_extract(provenance_json,'$.scope')='individual_work_revisit' AND outcome NOT IN ('returned','not_sent')",[&self.run],|r|r.get(0)).map_err(sql_error)?;
        Ok(
            json!({"management_admission_notes":self.journal_notes(),"backlog_reserved_candidates":self.backlog_reserved.load(std::sync::atomic::Ordering::Relaxed),"event_budget_policy":"at most half to retained candidate views; odd one-event invocations prefer retained work, even invocations recent discovery","retained_full_events_this_run":full,"partial_receipts_this_run":partial,"unconfirmed_management_reads":uncertain,"unconfirmed_read_semantics":"started with actual_sends=0 means unconfirmed, never proof of no send","revisited_work_attempts":revisit,"incomplete_revisited_results":revisit_incomplete,"retained_work":work,"token_attempts":self.budget.token_attempts(),"cached_examples":self.cached_examples.load(std::sync::atomic::Ordering::Relaxed)}),
        )
    }
    pub fn finish(&self, evidence: &Evidence) -> Result<serde_json::Value, CliError> {
        self.checkpoint(evidence)?;
        self.batch(|conn|{
            let mut progress=self.progress(conn)?;
            progress["selected_report_ids"]=json!(evidence.sampling.selected_report_ids.len());
            progress["incomplete_required_sources"]=json!(evidence.incomplete());
            let incomplete=evidence.incomplete()>0||progress["partial_receipts_this_run"].as_i64().unwrap_or(1)>0||progress["unconfirmed_management_reads"].as_i64().unwrap_or(1)>0||progress["incomplete_revisited_results"].as_i64().unwrap_or(1)>0;
            let state=if incomplete{"partial"}else{"complete"};progress["run_state"]=json!(state);
            conn.execute("UPDATE collection_runs SET ended_ms=?2,state=?3,provenance_json=json_set(provenance_json,'$.progress',json(?4)) WHERE run_id=?1",params![self.run,database::now_ms()?,state,progress.to_string()]).map_err(sql_error)?;
            Ok(progress)
        })
    }
}

impl airs_runtime_core::signal::sink::HistorySink for Sync {
    fn budget(&self) -> &std::sync::Arc<airs_runtime_core::read_budget::ReadBudget> {
        &self.budget
    }
    fn run_id(&self) -> &str {
        &self.run
    }
    fn checkpoint(&self, evidence: &Evidence) -> Result<(), CliError> {
        Sync::checkpoint(self, evidence)
    }
    fn current_event_limit(&self, limit: u64) -> u64 {
        Sync::current_event_limit(self, limit)
    }
    fn sessions_discovered(
        &self,
        keys: &[(String, String, String)],
        window: &airs_runtime_core::signal::evidence::Window,
    ) -> Result<(), CliError> {
        Sync::sessions_discovered(self, keys, window)
    }
    fn session_finished(
        &self,
        key: (&str, &str, &str),
        window: &airs_runtime_core::signal::evidence::Window,
        complete: bool,
    ) -> Result<(), CliError> {
        Sync::session_finished(self, key, window, complete)
    }
}

/// One explicit collection into the archive (formerly `airs runtime sync`; the service
/// runs it as a durable job from M3/M4).
pub struct SyncRequest {
    /// The admitted archive root.
    pub root: PathBuf,
    /// The requested half-open UTC range; `None` is the 24 hours before `now`.
    pub range: Option<(i64, i64)>,
    pub options: airs_runtime_core::scan_logs::CollectionOptions,
    pub backfill: bool,
    /// Count partial receipts and incomplete revisits as incomplete.
    pub strict: bool,
    /// The run ID to record (crash point C4: known durably before the archive commit);
    /// `None` mints one.
    pub run_id: Option<String>,
    /// The service's explicit fresh-token replay on a 401 (contract §10); off for the CLI.
    pub auth_replay: bool,
    /// Called at every Reports transport admission with its ID count (the service
    /// charges its durable daily allowance there); refusal stops that transport.
    pub reports_gate: Option<airs_runtime_core::read_budget::ReportsGate>,
    /// Checked before every send (the service's durable job cancellation); `None` for
    /// the CLI.
    pub admission_gate: Option<airs_runtime_core::read_budget::AdmissionGate>,
    /// The credential revision the service admitted this run under, recorded in the
    /// run's provenance before any send (contract §6.1); `None` for the CLI.
    pub credential_revision: Option<String>,
    /// The service's in-run transport admission fence; `None` for the CLI.
    pub send_fence: Option<SendFence>,
}

/// A finished collection: its summary document and whether it retained incomplete
/// observations (unknown health, or strict incompleteness).
pub struct SyncReport {
    pub run_id: String,
    pub document: serde_json::Value,
    pub observed_events: usize,
    pub incomplete_required_sources: usize,
    pub incomplete: bool,
}

/// A failed collection. Observations committed before the failure stay in the archive;
/// `final_status_saved` is false when even the run's final status could not be written.
pub struct SyncFailure {
    pub error: CliError,
    pub final_status_saved: bool,
}

impl From<CliError> for SyncFailure {
    fn from(error: CliError) -> Self {
        Self {
            error,
            final_status_saved: true,
        }
    }
}

/// Collect the tenant's recent runtime observations into its archive: management
/// discovery and sessions through the shared collector, due revisits, sparse cached
/// enrichment and transaction metadata, with every attempt admitted through one staged
/// read budget. Ordinary collection makes no Reports calls.
pub async fn run(
    ctx: &airs_runtime_core::signal::sink::CollectContext<'_>,
    tenant: &TenantContext,
    request: SyncRequest,
) -> Result<SyncReport, SyncFailure> {
    use airs_runtime_core::signal::collect;
    let now = database::now_ms()?;
    let target = request.range.unwrap_or((now - 86_400_000, now));
    let mut options = request.options;
    options.interval = u64::try_from(now.saturating_sub(target.0))
        .unwrap_or(u64::MAX)
        .div_ceil(3_600_000)
        .max(1);
    if options.interval > 87600 {
        return Err(CliError::usage(
            "--since exceeds the supported 87600-hour rolling query bound",
        )
        .into());
    }
    let budget = airs_runtime_core::read_budget::ReadBudget::with_stages(
        options.limits.seconds,
        options.limits.requests,
        options.limits.concurrency,
        ctx.session.clone(),
        [20, 70, 100],
    );
    if request.auth_replay {
        budget.enable_auth_replay();
    }
    if let Some(gate) = request.admission_gate.clone() {
        budget.set_admission_gate(gate);
    }
    if let Some(gate) = request.reports_gate.clone() {
        budget.set_reports_gate(gate);
    }
    let sync = std::sync::Arc::new(Sync::start(
        &request.root,
        tenant,
        target,
        (
            Duration::from_secs(options.limits.seconds),
            request.backfill,
            options.app_ids.clone(),
            request.run_id.clone(),
            request.credential_revision.clone(),
        ),
        (ctx.secrets, ctx.payload_spans),
        ctx.session,
        budget,
    )?);
    if let Some(fence) = request.send_fence {
        let _ = sync.fence.set(fence);
    }
    sync.journal();
    let collected = async {
        let backlog = sync.prepare_backlog(options.max_events, options.max_reports)?;
        let (backlog, old_reports) = Sync::split_backlog(backlog);
        let collected = collect::collect_management(ctx, tenant, &options, &*sync).await?;
        let mut evidence = collected.evidence;
        sync.revisit(ctx, tenant, &evidence, backlog).await?;
        sync.enrich(ctx, tenant, &mut evidence, old_reports).await?;
        collect::complete_management(
            ctx,
            &options,
            &*sync,
            collected.clients.as_ref(),
            &collected.telemetry_buckets,
            &mut evidence,
        )
        .await?;
        let progress = sync.finish(&evidence)?;
        Ok::<_, CliError>((evidence, progress))
    }
    .await;
    let (evidence, progress) = match collected {
        Ok(collected) => collected,
        Err(error) => {
            return Err(SyncFailure {
                error,
                final_status_saved: sync.abort().is_ok(),
            });
        }
    };
    let document = json!({"run_id":sync.run,"state":progress["run_state"],"requested_since_ms":target.0,"requested_until_ms":target.1,"actual_rolling_query":evidence.window,"observed_events":evidence.events.len(),"admitted_transport_attempts":sync.budget.attempts(),"progress":progress,"collection_unavailable":evidence.health_unknown(),"incomplete_required_sources":evidence.incomplete(),"coverage_semantics":"traversed as of collection; local absolute filtering; no provider watermark","backfill":request.backfill});
    let strict_incomplete = request.strict
        && (evidence.incomplete() > 0
            || progress["partial_receipts_this_run"].as_i64().unwrap_or(1) > 0
            || progress["incomplete_revisited_results"]
                .as_i64()
                .unwrap_or(1)
                > 0);
    Ok(SyncReport {
        run_id: sync.run.clone(),
        document,
        observed_events: evidence.events.len(),
        incomplete_required_sources: evidence.incomplete(),
        incomplete: evidence.health_unknown() || strict_incomplete,
    })
}
