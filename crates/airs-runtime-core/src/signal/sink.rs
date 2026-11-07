//! The narrow seam between the shared live collector and persistent history (plan
//! "Workspace boundary"): the collector never names the history store. The standalone
//! `airs runtime report` passes no sink and persists nothing; the server's history
//! store implements [`HistorySink`] over its archive.

use super::evidence::{Evidence, Window};
use crate::error::CliError;
use crate::read_budget::ReadBudget;
use crate::secrets::SecretSet;
use std::sync::Arc;

/// What one collection run reads from its caller: the cancellation session, the
/// secrets learned so far (every OAuth lease is added), the payload spans withheld
/// from evidence, and the version recorded in the evidence document. Built per
/// request/job; never process-global.
pub struct CollectContext<'a> {
    pub session: &'a scm::Session,
    pub secrets: &'a SecretSet,
    pub payload_spans: &'a super::privacy::PayloadSpans,
    pub version: &'a str,
}

/// Persistent history for one collection run. Each call commits a bounded,
/// privacy-projected checkpoint; a failure stops the collection with a fixed message.
pub trait HistorySink: Sync {
    /// The run's shared read budget (every outbound attempt is admitted through it).
    fn budget(&self) -> &Arc<ReadBudget>;
    /// The run identifier recorded as the evidence `collection_id`.
    fn run_id(&self) -> &str;
    /// Commit the observations collected so far.
    fn checkpoint(&self, evidence: &Evidence) -> Result<(), CliError>;
    /// The event cap left after work the sink already reserved.
    fn current_event_limit(&self, limit: u64) -> u64;
    /// Record the sessions discovered for `window` before their details are read.
    fn sessions_discovered(
        &self,
        keys: &[(String, String, String)],
        window: &Window,
    ) -> Result<(), CliError>;
    /// Record one session's detail traversal (`complete` when every action was kept).
    fn session_finished(
        &self,
        key: (&str, &str, &str),
        window: &Window,
        complete: bool,
    ) -> Result<(), CliError>;
}
