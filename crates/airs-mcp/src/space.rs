//! The disk reserve (contract §6 limits, §9.5; M8 review 1, fork C gap).
//!
//! `reserve = 2 × (control + archive bytes) + 2 × 64 MiB + outstanding transfer
//! reservations + 64 MiB`. With `free` the bytes `statvfs` reports available on the
//! data volume:
//!
//! - `free < reserve + 512 MiB`: **publication pauses**. Report bundles are not staged,
//!   and uploads are neither begun nor written. Collection continues.
//! - `free < reserve`: **collection pauses too**. Collection jobs and receipt ingestion
//!   write nothing.
//!
//! A paused job ends its run `blocked_policy` and is requeued by the worker's policy
//! pass once space is freed, within its original deadline (contract §10). A paused
//! synchronous tool answers `storage_unavailable` and writes nothing.
//!
//! **Fail closed.** The level is `unknown` when any part of the measurement fails: the
//! free bytes (`statvfs`), the store bytes (a directory that cannot be listed or
//! inspected, or more entries than the sampling bound), or the outstanding upload
//! reservations (the control-store query). It is also `unknown` when a request
//! handler's or a scrape's bounded wait for the shared sampler expires (M8 review 2
//! findings 7 and 8). While it is `unknown`, every allocating class pauses as if the
//! volume were at its lowest level.
//!
//! **Exemptions.** Only operations that do not allocate run regardless:
//! - reads and job inspection (`Class::Other`);
//! - the space-freeing lifecycle actions `prune`, `compact` and `artifact_delete`
//!   ([`lifecycle_class`]).
//!
//! Every other `lifecycle_execute` action (`export`, `import_finalize`, `recover`, and
//! any action this version does not know) is publication-class, because it allocates.

use std::path::Path;
use std::sync::Arc;

pub const MIB: u64 = 1 << 20;
/// The headroom above the reserve below which publication pauses.
pub const PUBLICATION_HEADROOM: u64 = 512 * MIB;

/// The reserve for `db_bytes` of stores and `transfer_reserved` bytes of open uploads.
pub fn reserve(db_bytes: u64, transfer_reserved: u64) -> u64 {
    db_bytes
        .saturating_mul(2)
        .saturating_add(2 * 64 * MIB)
        .saturating_add(transfer_reserved)
        .saturating_add(64 * MIB)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Normal,
    PublicationPaused,
    CollectionPaused,
    /// The volume could not be measured. This fails closed (M8 review 2 watchdog):
    /// publication and collection both pause until a measurement succeeds; lifecycle
    /// operations, which free space, still run.
    Unknown,
}

impl Level {
    pub fn of(free: Option<u64>, reserve: u64) -> Self {
        match free {
            None => Self::Unknown,
            Some(f) if f < reserve => Self::CollectionPaused,
            Some(f) if f < reserve.saturating_add(PUBLICATION_HEADROOM) => Self::PublicationPaused,
            Some(_) => Self::Normal,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::PublicationPaused => "publication_paused",
            Self::CollectionPaused => "collection_paused",
            Self::Unknown => "unknown",
        }
    }
}

/// One measurement of the data volume against the reserve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Space {
    pub free: Option<u64>,
    pub reserve: u64,
    pub level: Level,
}

impl Space {
    pub fn allows_publication(&self) -> bool {
        matches!(self.level, Level::Normal)
    }

    pub fn allows_collection(&self) -> bool {
        matches!(self.level, Level::Normal | Level::PublicationPaused)
    }

    /// Whether a job of `operation` may run now.
    pub fn admits(&self, operation: &str) -> bool {
        self.admits_class(class(operation))
    }

    fn admits_class(&self, c: Class) -> bool {
        match c {
            Class::Publication => self.allows_publication(),
            Class::Collection => self.allows_collection(),
            Class::Other => true,
        }
    }

    /// Whether a job may run now, with its `lifecycle_execute` action when known. The
    /// worker always passes the action: unreadable parameters arrive as `Some("")`,
    /// which is publication-class. The policy pass cannot read parameters under its
    /// transaction and passes `None`. A blocked lifecycle job is then requeued, and the
    /// worker re-checks it with its action before anything is written.
    pub fn admits_job(&self, operation: &str, action: Option<&str>) -> bool {
        if operation == "lifecycle_execute" {
            return action.is_none_or(|a| self.admits_class(lifecycle_class(a)));
        }
        self.admits(operation)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Writes bundles or staging (reports, revisions, uploads, imports, exports).
    Publication,
    /// Writes the archive (collection, enrichment, receipt ingestion).
    Collection,
    Other,
}

/// The class of an operation (job or tool) for the reserve.
pub fn class(operation: &str) -> Class {
    match operation {
        "reports_create"
        | "daily_report"
        | "catch_up"
        | "revision"
        | "transfer_upload_begin"
        | "transfer_upload_chunk" => Class::Publication,
        "collection_start" | "enrichment_start" | "daily_enrichment" | "receipts_ingest" => {
            Class::Collection
        }
        _ => Class::Other,
    }
}

/// The class of one `lifecycle_execute` action. Only the actions that free space are
/// exempt; an action that allocates, or one this version does not know, is
/// publication-class.
pub fn lifecycle_class(action: &str) -> Class {
    match action {
        "prune" | "compact" | "artifact_delete" => Class::Other,
        _ => Class::Publication,
    }
}

/// Where free space comes from: `statvfs` in production; tests inject their own.
pub type Probe = Arc<dyn Fn(&Path) -> Option<u64> + Send + Sync>;

/// The bytes `statvfs` reports available to the service on the filesystem of `dir`.
pub fn statvfs_free(dir: &Path) -> Option<u64> {
    crate::metrics::volume_bytes(dir).map(|(_, free)| free)
}

pub fn default_probe() -> Probe {
    Arc::new(statvfs_free)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reserve_and_its_two_thresholds() {
        let r = reserve(10 * MIB, 3 * MIB);
        assert_eq!(r, 20 * MIB + 128 * MIB + 3 * MIB + 64 * MIB);
        assert_eq!(Level::of(Some(r + PUBLICATION_HEADROOM), r), Level::Normal);
        assert_eq!(
            Level::of(Some(r + PUBLICATION_HEADROOM - 1), r),
            Level::PublicationPaused
        );
        assert_eq!(Level::of(Some(r), r), Level::PublicationPaused);
        assert_eq!(Level::of(Some(r - 1), r), Level::CollectionPaused);
        assert_eq!(Level::of(None, r), Level::Unknown);
        let s = |level| Space {
            free: None,
            reserve: r,
            level,
        };
        assert!(s(Level::PublicationPaused).admits("collection_start"));
        assert!(!s(Level::PublicationPaused).admits("reports_create"));
        assert!(!s(Level::CollectionPaused).admits("collection_start"));
        assert!(!s(Level::CollectionPaused).admits("transfer_upload_begin"));
        assert!(s(Level::CollectionPaused).admits("jobs_get"));
        // An unmeasurable volume fails closed for writes, open for lifecycle and reads.
        assert!(!s(Level::Unknown).admits("reports_create"));
        assert!(!s(Level::Unknown).admits("collection_start"));
        assert!(s(Level::Unknown).admits("jobs_get"));
        // M8 review 2 finding 7: lifecycle actions by what they do.
        for level in [
            Level::PublicationPaused,
            Level::CollectionPaused,
            Level::Unknown,
        ] {
            for freeing in ["prune", "compact", "artifact_delete"] {
                assert!(
                    s(level).admits_job("lifecycle_execute", Some(freeing)),
                    "{freeing}"
                );
            }
            for allocating in ["export", "import_finalize", "recover", "", "unknown_action"] {
                assert!(
                    !s(level).admits_job("lifecycle_execute", Some(allocating)),
                    "{allocating} at {level:?}"
                );
            }
        }
        assert!(s(Level::Normal).admits_job("lifecycle_execute", Some("export")));
        // The policy pass (no parameters) requeues; the worker re-checks.
        assert!(s(Level::Unknown).admits_job("lifecycle_execute", None));
        assert!(!s(Level::Unknown).admits_job("reports_create", None));
    }
}
