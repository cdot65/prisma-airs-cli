//! Invocation-local bounds for discovery/result/report reads, including OAuth sends.
use futures_util::future::BoxFuture;
use scm::transport::{Transport, TransportRequest, TransportResponse};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::Semaphore, time::Instant};

/// Optional durable journal for explicit history collection. It receives a bounded
/// fixed-operation descriptor, never headers, bodies, credentials or a full URL.
pub trait ReadJournal: Send + Sync {
    fn begin(&self, operation: &str, query: &[(String, String)]) -> scm::Result<String>;
    fn end(
        &self,
        id: &str,
        status: Option<u16>,
        sent: bool,
        retry_after: Option<u64>,
    ) -> scm::Result<()>;
}

/// A Reports admission gate: called with the transport's ID count immediately before
/// each Reports send; `false` refuses that send (the service's durable daily allowance).
pub type ReportsGate = Arc<dyn Fn(u64) -> bool + Send + Sync>;

/// An admission gate: called immediately before every send (API reads, OAuth token
/// requests, replays, Reports); `false` refuses that send as cancelled (the service
/// checks its durable job cancellation here, so a cancellation committed between any
/// two sends stops the next one).
pub type AdmissionGate = Arc<dyn Fn() -> bool + Send + Sync>;

/// One logical read and its single fresh-token replay (contract §10): every transport
/// sent while one [`ReadBudget::run`] is polled carries the same chain, and distinct
/// logical reads never share one. The journal records the chain in the transport
/// descriptor, so "a read refused twice" means a read and its own replay, never two
/// distinct reads that happen to share an operation and query.
struct Chain {
    id: u64,
    /// A management API transport of this chain answered 401: the only refusal the
    /// fresh-token replay may answer (a scanner Results/Reports 401, authenticated by a
    /// static key, is final and never replayed).
    management_401: std::sync::atomic::AtomicBool,
}
tokio::task_local! {
    static CHAIN: Arc<Chain>;
}

/// Whether `operation` is a management API read, the only kind eligible for the
/// fresh-token replay: not a token request, not a scanner (`airs.scan.*`) read.
fn replay_eligible(operation: &str) -> bool {
    operation != "oauth.token" && !operation.starts_with("airs.scan.")
}

pub struct ReadBudget {
    started: Instant,
    seconds: u64,
    stages: [u64; 3],
    max_requests: u64,
    session: scm::Session,
    state: Mutex<State>,
    slots: Semaphore,
    journal: Mutex<Option<std::sync::Weak<dyn ReadJournal>>>,
    /// The service's explicit fresh-token replay (contract §10): off for the CLI.
    auth_replay: std::sync::atomic::AtomicBool,
    reports_gate: Mutex<Option<ReportsGate>>,
    admission_gate: Mutex<Option<AdmissionGate>>,
    chains: std::sync::atomic::AtomicU64,
}
struct State {
    stage: u8,
    attempts: u64,
    /// Admitted OAuth token requests (`oauth.token`), included in `attempts`.
    token_attempts: u64,
    /// Explicit 401 replays performed (each also an admitted attempt).
    auth_replays: u64,
    report_attempts: u64,
    report_ids: BTreeSet<String>,
    reports_stopped: bool,
    lookup_rows: u64,
    lookup_bytes: u64,
}
impl ReadBudget {
    pub fn new(seconds: u64, requests: u64, concurrency: u64, session: scm::Session) -> Arc<Self> {
        Self::with_stages(seconds, requests, concurrency, session, [40, 70, 100])
    }
    pub fn with_stages(
        seconds: u64,
        requests: u64,
        concurrency: u64,
        session: scm::Session,
        stages: [u64; 3],
    ) -> Arc<Self> {
        assert!(stages[0] <= stages[1] && stages[1] <= stages[2] && stages[2] == 100);
        Arc::new(Self {
            started: Instant::now(),
            seconds,
            stages,
            max_requests: requests,
            session,
            state: Mutex::new(State {
                stage: 0,
                attempts: 0,
                token_attempts: 0,
                auth_replays: 0,
                report_attempts: 0,
                report_ids: BTreeSet::new(),
                reports_stopped: false,
                lookup_rows: 0,
                lookup_bytes: 0,
            }),
            slots: Semaphore::new(concurrency as usize),
            journal: Mutex::new(None),
            auth_replay: std::sync::atomic::AtomicBool::new(false),
            reports_gate: Mutex::new(None),
            admission_gate: Mutex::new(None),
            chains: std::sync::atomic::AtomicU64::new(0),
        })
    }
    pub fn journal(&self, journal: std::sync::Weak<dyn ReadJournal>) {
        *self
            .journal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(journal);
    }
    pub fn stage(&self, stage: u8) {
        let mut s = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        s.stage = s.stage.max(stage.min(2));
    }
    fn fraction(&self) -> u64 {
        let s = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.stages[usize::from(s.stage)]
    }
    pub fn deadline(&self) -> Instant {
        self.started + Duration::from_millis(self.seconds * 1000 * self.fraction() / 100)
    }
    /// Admitted OAuth token requests (a subset of [`ReadBudget::attempts`]).
    pub fn token_attempts(&self) -> u64 {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .token_attempts
    }
    pub fn attempts(&self) -> u64 {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .attempts
    }
    pub fn lookup_capacity(&self) -> bool {
        let s = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        s.lookup_rows < 10000 && s.lookup_bytes < 32 * 1024 * 1024
    }
    #[cfg(test)]
    pub fn report_attempts(&self) -> u64 {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .report_attempts
    }
    pub fn retain_lookup_row(&self, bytes: usize) -> bool {
        let mut s = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
        if s.lookup_rows >= 10000 || s.lookup_bytes.saturating_add(bytes) > 32 * 1024 * 1024 {
            return false;
        }
        s.lookup_rows += 1;
        s.lookup_bytes += bytes;
        true
    }
    fn stop_reports(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reports_stopped = true;
    }
    fn admit(&self, report_ids: Option<&[String]>) -> scm::Result<()> {
        if self.session.is_cancelled() {
            return Err(scm::Error::new(scm::ErrorKind::Cancelled));
        }
        let gate = self
            .admission_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if let Some(gate) = gate
            && !gate()
        {
            self.session.cancel();
            return Err(scm::Error::new(scm::ErrorKind::Cancelled));
        }
        if Instant::now() >= self.deadline() {
            return Err(scm::Error::new(scm::ErrorKind::Deadline));
        }
        let mut s = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fraction = self.stages[usize::from(s.stage)];
        if s.attempts >= self.max_requests * fraction / 100 {
            return Err(scm::Error::new(scm::ErrorKind::Limit));
        }
        if let Some(ids) = report_ids {
            let unique: BTreeSet<_> = ids.iter().cloned().collect();
            if s.reports_stopped
                || s.report_attempts >= 2
                || ids.is_empty()
                || ids.len() > 5
                || s.report_ids.union(&unique).count() > 10
            {
                return Err(scm::Error::new(scm::ErrorKind::Limit));
            }
            s.report_attempts += 1;
            s.report_ids.extend(unique);
        }
        s.attempts += 1;
        Ok(())
    }
    /// Enable the explicit fresh-token replay: a management read refused with 401 is
    /// sent exactly once more, after the SDK invalidated the rejected lease, so the
    /// replay carries a fresh token. SDK retries stay at zero; a 403 is never replayed;
    /// a token request or a scanner (Results/Reports) read is never replayed; the replay
    /// is admitted (and journaled, in the same chain) like any send, and a second 401 is
    /// final.
    pub fn enable_auth_replay(&self) {
        self.auth_replay
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
    /// Install the admission gate checked before every send.
    pub fn set_admission_gate(&self, gate: AdmissionGate) {
        *self
            .admission_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(gate);
    }
    /// Install the Reports admission gate.
    pub fn set_reports_gate(&self, gate: ReportsGate) {
        *self
            .reports_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(gate);
    }
    /// Explicit 401 replays performed.
    pub fn auth_replays(&self) -> u64 {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .auth_replays
    }
    /// Slot covers the entire SDK operation, including authentication and body decode.
    /// Nested OAuth sends share attempt admission without taking another operation slot.
    pub async fn run<T, F, Fut>(&self, read: F) -> scm::Result<T>
    where
        F: Fn(scm::RequestOptions) -> Fut,
        Fut: std::future::Future<Output = scm::Result<T>>,
    {
        self.run_slice(None, read).await
    }
    /// A per-operation slice starts after acquiring a slot, leaving queued peers time.
    pub async fn run_slice<T, F, Fut>(&self, slice: Option<Duration>, read: F) -> scm::Result<T>
    where
        F: Fn(scm::RequestOptions) -> Fut,
        Fut: std::future::Future<Output = scm::Result<T>>,
    {
        if self.session.is_cancelled() {
            return Err(scm::Error::new(scm::ErrorKind::Cancelled));
        }
        let chain = Arc::new(Chain {
            id: self
                .chains
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1,
            management_401: std::sync::atomic::AtomicBool::new(false),
        });
        CHAIN
            .scope(chain.clone(), self.run_chain(slice, &chain, read))
            .await
    }
    async fn run_chain<T, F, Fut>(
        &self,
        slice: Option<Duration>,
        chain: &Chain,
        read: F,
    ) -> scm::Result<T>
    where
        F: Fn(scm::RequestOptions) -> Fut,
        Fut: std::future::Future<Output = scm::Result<T>>,
    {
        let deadline = self.deadline();
        tokio::select! {
            biased;
            ()=self.session.cancelled()=>Err(scm::Error::new(scm::ErrorKind::Cancelled)),
            result=tokio::time::timeout_at(deadline,async {
                let _slot=self.slots.acquire().await.map_err(|_|scm::Error::new(scm::ErrorKind::Cancelled))?;
                let remaining=deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero(){return Err(scm::Error::new(scm::ErrorKind::Deadline));}
                let options=scm::RequestOptions::with_timeout(remaining)?.with_retries(0)?.without_response_defaults();
                let deadline = slice.map_or(deadline, |slice| deadline.min(Instant::now() + slice));
                let first = tokio::time::timeout_at(deadline, read(options)).await.map_err(|_| scm::Error::new(scm::ErrorKind::Deadline))?;
                match first {
                    // Only a management API response refused with 401 is replayed: a
                    // refused token request (`Authentication`) is a credential outcome,
                    // and a scanner 401 (static key) is final; neither is replayed.
                    Err(e) if e.kind() == scm::ErrorKind::Http && e.status() == Some(401) && self.auth_replay.load(std::sync::atomic::Ordering::SeqCst) && chain.management_401.load(std::sync::atomic::Ordering::SeqCst) && !self.session.is_cancelled() => {
                        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).auth_replays += 1;
                        let remaining = deadline.saturating_duration_since(Instant::now());
                        if remaining.is_zero() { return Err(scm::Error::new(scm::ErrorKind::Deadline)); }
                        let options = scm::RequestOptions::with_timeout(remaining)?.with_retries(0)?.without_response_defaults();
                        tokio::time::timeout_at(deadline, read(options)).await.map_err(|_| scm::Error::new(scm::ErrorKind::Deadline))?
                    }
                    other => other,
                }
            })=>result.map_err(|_|scm::Error::new(scm::ErrorKind::Deadline))?
        }
    }
    pub fn wrap(self: &Arc<Self>, inner: Arc<dyn Transport>) -> Arc<dyn Transport> {
        Arc::new(BoundedTransport {
            inner,
            budget: self.clone(),
        })
    }
}
struct BoundedTransport {
    inner: Arc<dyn Transport>,
    budget: Arc<ReadBudget>,
}
impl Transport for BoundedTransport {
    fn send<'a>(
        &'a self,
        request: TransportRequest,
    ) -> BoxFuture<'a, scm::Result<TransportResponse>> {
        Box::pin(async move {
            // Inspect only the fixed route and parsed IDs, never print URL/query data.
            let reports = request.operation() == "airs.scan.reports";
            let eligible = replay_eligible(request.operation());
            let ids: Vec<String> = if reports {
                request
                    .url()
                    .query_pairs()
                    .filter(|(key, _)| key == "report_ids")
                    .flat_map(|(_, value)| value.split(',').map(str::to_owned).collect::<Vec<_>>())
                    .collect()
            } else {
                Vec::new()
            };
            let journal = if request.operation().starts_with("airs.scan.") {
                None
            } else {
                self.budget
                    .journal
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .as_ref()
                    .and_then(std::sync::Weak::upgrade)
            };
            let ticket = if let Some(journal) = &journal {
                let query = request
                    .url()
                    .query_pairs()
                    .filter(|(key, _)| {
                        matches!(
                            key.as_ref(),
                            "application_id"
                                | "application_name"
                                | "app_id"
                                | "app_name"
                                | "session_id"
                                | "scan_id"
                                | "scan_sub_req_id"
                                | "time_interval"
                                | "time_unit"
                                | "limit"
                                | "offset"
                        )
                    })
                    .take(32)
                    .map(|(k, v)| (k.into_owned(), v.chars().take(4096).collect()))
                    .chain(
                        CHAIN
                            .try_with(|c| c.id)
                            .ok()
                            .map(|id| ("replay_chain".to_owned(), id.to_string())),
                    )
                    .collect::<Vec<_>>();
                Some(journal.begin(request.operation(), &query)?)
            } else {
                None
            };
            if let Err(error) = self.budget.admit(reports.then_some(ids.as_slice())) {
                if let (Some(journal), Some(ticket)) = (&journal, &ticket) {
                    journal.end(ticket, None, false, None)?;
                }
                return Err(error);
            }
            if reports {
                let gate = self
                    .budget
                    .reports_gate
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                if let Some(gate) = gate
                    && !gate(ids.len() as u64)
                {
                    self.budget.stop_reports();
                    return Err(scm::Error::new(scm::ErrorKind::Limit));
                }
            }
            if request.operation() == "oauth.token" {
                self.budget
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .token_attempts += 1;
            }
            let response = tokio::select! {
                ()=self.budget.session.cancelled()=>Err(scm::Error::new(scm::ErrorKind::Cancelled)),
                response=tokio::time::timeout_at(self.budget.deadline(),self.inner.send(request))=>response.unwrap_or_else(|_|Err(scm::Error::new(scm::ErrorKind::Deadline)))
            };
            let status = match &response {
                Ok(r) => Some(r.status().as_u16()),
                Err(e) => e.status(),
            };
            if status == Some(401) && eligible {
                let _ = CHAIN.try_with(|c| {
                    c.management_401
                        .store(true, std::sync::atomic::Ordering::SeqCst)
                });
            }
            let limited = status == Some(429);
            if reports && limited {
                self.budget.stop_reports();
            }
            if let (Some(journal), Some(ticket)) = (&journal, &ticket) {
                let status = match &response {
                    Ok(r) => Some(r.status().as_u16()),
                    Err(e) => e.status(),
                };
                let retry_after = match &response {
                    Ok(r) => crate::clients::retry_after_seconds(
                        r.headers().get("retry-after").and_then(|v| v.to_str().ok()),
                        std::time::SystemTime::now(),
                    ),
                    Err(_) => None,
                };
                journal.end(ticket, status, true, retry_after)?;
            }
            response
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lookup_storage_budget_is_shared_by_every_scanner_client() {
        let budget = ReadBudget::new(10, 100, 4, scm::Session::new());
        assert!(budget.retain_lookup_row(32 * 1024 * 1024 - 1));
        assert!(budget.retain_lookup_row(1));
        assert!(!budget.lookup_capacity());
        assert!(!budget.retain_lookup_row(1));
        let budget = ReadBudget::new(10, 100, 4, scm::Session::new());
        for _ in 0..10000 {
            assert!(budget.retain_lookup_row(0));
        }
        assert!(!budget.retain_lookup_row(0));
        assert!(!budget.lookup_capacity());
    }
    #[test]
    fn stage_reservations_count_actual_attempts_and_carry_unused_allowance() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let b = ReadBudget::new(10, 10, 4, scm::Session::new());
            for _ in 0..4 {
                b.admit(None).unwrap();
            }
            assert!(b.admit(None).is_err());
            b.stage(1);
            for _ in 0..3 {
                b.admit(None).unwrap();
            }
            assert!(b.admit(None).is_err());
            b.stage(2);
            for _ in 0..3 {
                b.admit(None).unwrap();
            }
            assert_eq!(b.attempts(), 10);
            assert!(b.admit(None).is_err());
        });
    }
    #[test]
    fn sdk_operation_slots_cover_decoding_and_cancel_waiters() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Active(Arc<AtomicUsize>);
        impl Drop for Active {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let session = scm::Session::new();
            let b = ReadBudget::new(5, 100, 2, session.clone());
            b.stage(2);
            let active = Arc::new(AtomicUsize::new(0));
            let maximum = Arc::new(AtomicUsize::new(0));
            let jobs = (0..6).map(|_| {
                let b = b.clone();
                let active = active.clone();
                let maximum = maximum.clone();
                async move {
                    b.run(|_| {
                        let (active, maximum) = (active.clone(), maximum.clone());
                        async move {
                            let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                            maximum.fetch_max(count, Ordering::SeqCst);
                            let _active = Active(active);
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            Ok(())
                        }
                    })
                    .await
                    .unwrap();
                }
            });
            futures_util::future::join_all(jobs).await;
            assert_eq!(maximum.load(Ordering::SeqCst), 2);
            assert_eq!(active.load(Ordering::SeqCst), 0);
            session.cancel();
            assert_eq!(
                b.run(|_| async { Ok(()) }).await.unwrap_err().kind(),
                scm::ErrorKind::Cancelled
            );
        });
    }
}

#[cfg(test)]
mod report_guard_tests {
    use super::*;
    use crate::tenant::config::{ConfigValue, KEYS, TenantConfig};
    use airs_test_harness::{FixtureConfig, FixtureServer, Planned, Route};
    #[test]
    fn repeated_sdk_report_operations_share_two_actual_transport_attempts() {
        let server = FixtureServer::start();
        server.add_tenant(
            "100",
            "client-100",
            "FAKE-SECRET-100",
            Some("FAKE-APIKEY-100"),
            None,
        );
        server.set_default(
            Route::ScanReports,
            Planned::json(200, &serde_json::json!([])),
        );
        let values = FixtureConfig::standard(&server.base_url(), "100").to_value();
        let config = TenantConfig::from_values(KEYS.iter().filter_map(|key| {
            values
                .get(*key)
                .and_then(serde_json::Value::as_str)
                .map(|v| (*key, ConfigValue::Text(v.into())))
        }));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for limited in [false, true] {
            server.clear_requests();
            server.set_default(
                Route::ScanReports,
                Planned::json(if limited { 429 } else { 200 }, &serde_json::json!([]))
                    .with_retry_after(3600),
            );
            rt.block_on(async {
                let session = scm::Session::new();
                let budget = ReadBudget::new(5, 100, 4, session.clone());
                budget.stage(2);
                let (scanner, _) =
                    crate::clients::build_scanner_bounded(&config, &session, None, Some(&budget))
                        .unwrap();
                for i in 0..3 {
                    let ids = vec![format!("report-{i}")];
                    let ids = &ids;
                    let scanner = &scanner;
                    let result =
                        budget
                            .run(|opts| async move {
                                scanner.query_by_report_ids(ids, Some(&opts)).await
                            })
                            .await;
                    if limited && i == 0 {
                        assert_eq!(result.unwrap_err().status(), Some(429));
                    } else if !limited && i < 2 {
                        result.unwrap();
                    } else {
                        assert_eq!(result.unwrap_err().kind(), scm::ErrorKind::Limit);
                    }
                }
                let expected = if limited { 1 } else { 2 };
                assert_eq!(budget.report_attempts(), expected);
                assert_eq!(budget.attempts(), expected);
                assert_eq!(
                    server.requests_for(Route::ScanReports).len() as u64,
                    expected
                );
            });
        }
        server.assert_no_unknown_routes();
    }
}

#[cfg(test)]
mod replay_chain_tests {
    use super::*;
    use crate::tenant::config::{ConfigValue, KEYS, TenantConfig};
    use airs_test_harness::{FixtureConfig, FixtureServer, Planned, Route, Rx};

    /// Records every journaled transport's operation, its `replay_chain` and its
    /// descriptor's query keys.
    type Row = (String, Option<String>, Vec<String>);
    #[derive(Default)]
    struct Recorder(Mutex<Vec<Row>>);
    impl ReadJournal for Recorder {
        fn begin(&self, operation: &str, query: &[(String, String)]) -> scm::Result<String> {
            let chain = query
                .iter()
                .find(|(k, _)| k == "replay_chain")
                .map(|(_, v)| v.clone());
            let keys = query.iter().map(|(k, _)| k.clone()).collect();
            let mut rows = self.0.lock().unwrap();
            rows.push((operation.to_owned(), chain, keys));
            Ok(rows.len().to_string())
        }
        fn end(&self, _: &str, _: Option<u16>, _: bool, _: Option<u64>) -> scm::Result<()> {
            Ok(())
        }
    }

    fn config(server: &FixtureServer) -> TenantConfig {
        server.add_tenant(
            "100",
            "client-100",
            "FAKE-SECRET-100",
            Some("FAKE-APIKEY-100"),
            None,
        );
        let values = FixtureConfig::standard(&server.base_url(), "100").to_value();
        TenantConfig::from_values(KEYS.iter().filter_map(|key| {
            values
                .get(*key)
                .and_then(serde_json::Value::as_str)
                .map(|v| (*key, ConfigValue::Text(v.into())))
        }))
    }

    /// M4 review 8 finding 1, at the source: two concurrent reads with an identical
    /// query each get a first 401. Each is its own replay chain: the first attempt and
    /// its single replay share one chain, the two reads never do, so the journal can
    /// tell "two reads refused once" from "one read refused twice".
    #[test]
    fn each_logical_read_and_its_replay_share_one_chain_and_distinct_reads_never_do() {
        let server = FixtureServer::start();
        server.seed_runtime();
        let config = config(&server);
        let tx = Route::Runtime(Rx::SessionTransaction);
        server.push_once(
            tx,
            Planned::status(401).with_delay(Duration::from_millis(10)),
        );
        server.push_once(
            tx,
            Planned::status(401).with_delay(Duration::from_millis(20)),
        );
        server.set_default(
            Route::Token,
            Planned::builtin().with_delay(Duration::from_millis(40)),
        );
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let session = scm::Session::new();
            let budget = ReadBudget::new(10, 100, 4, session.clone());
            budget.stage(2);
            budget.enable_auth_replay();
            let recorder = Arc::new(Recorder::default());
            let journal: Arc<dyn ReadJournal> = recorder.clone();
            budget.journal(Arc::downgrade(&journal));
            let clients = crate::clients::ManagementClients::build_for_bounded(
                &config,
                &session,
                &crate::error::SecretSet::default(),
                None,
                crate::clients::Planes {
                    dashboard: true,
                    dlp: false,
                },
                Some(&budget),
            )
            .unwrap();
            let dashboard = clients.management.dashboard();
            let q = scm::airs::management::DashboardSessionTransactionQuery::new(
                "pan_session",
                "uuid-shared",
                "A & B",
                "scan-1",
                0,
            );
            let read = || {
                let (dashboard, q, budget) = (&dashboard, &q, &budget);
                async move {
                    budget
                        .run(|opts| async move { dashboard.session_transaction(q, Some(&opts)).await })
                        .await
                }
            };
            let (a, b) = futures_util::future::join(read(), read()).await;
            a.unwrap();
            b.unwrap();
            assert_eq!(budget.auth_replays(), 2);
            let rows = recorder.0.lock().unwrap().clone();
            let reads: Vec<_> = rows
                .iter()
                .filter(|(op, _, _)| op == "airs.management.dashboard.session_transaction")
                .collect();
            for (_, _, keys) in &reads {
                for key in ["session_id", "app_id", "app_name", "scan_id", "scan_sub_req_id"] {
                    assert!(
                        keys.iter().any(|k| k == key),
                        "the descriptor keeps the read's full identity ({key}): {keys:?}"
                    );
                }
            }
            let chains: Vec<_> = reads
                .iter()
                .map(|(_, c, _)| c.clone().expect("every transport of a read carries its chain"))
                .collect();
            assert_eq!(chains.len(), 4, "{rows:?}");
            let distinct: BTreeSet<_> = chains.iter().collect();
            assert_eq!(distinct.len(), 2, "two reads, two chains: {rows:?}");
            for chain in &distinct {
                assert_eq!(
                    chains.iter().filter(|c| c == chain).count(),
                    2,
                    "a read and its own replay share a chain: {rows:?}"
                );
            }
        });
        assert_eq!(server.requests_for(tx).len(), 4);
    }

    /// M4 review 8 finding 2, at the source: with the service's replay enabled, a
    /// scanner Results 401 (static key) is final: one transport, no replay, no token.
    #[test]
    fn a_scanner_401_is_never_replayed_even_with_the_service_replay() {
        let server = FixtureServer::start();
        let config = config(&server);
        server.push_once(Route::ScanResults, Planned::status(401));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let session = scm::Session::new();
            let budget = ReadBudget::new(10, 100, 4, session.clone());
            budget.stage(2);
            budget.enable_auth_replay();
            let (scanner, _) =
                crate::clients::build_scanner_bounded(&config, &session, None, Some(&budget))
                    .unwrap();
            let ids = vec!["00000000-0000-4000-8000-000000000001".to_owned()];
            let (scanner, ids) = (&scanner, &ids);
            let result = budget
                .run(|opts| async move { scanner.query_by_scan_ids(ids, Some(&opts)).await })
                .await;
            assert_eq!(result.unwrap_err().status(), Some(401));
            assert_eq!(budget.auth_replays(), 0);
            assert_eq!(budget.attempts(), 1);
        });
        assert_eq!(server.requests_for(Route::ScanResults).len(), 1);
        assert_eq!(server.requests_for(Route::Token).len(), 0);
    }
}
