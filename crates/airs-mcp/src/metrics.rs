//! Operational metrics on the private health listener (plan M8.1; contract §9.5).
//!
//! `GET /metrics` returns Prometheus text with a fixed set of series: readiness,
//! maintenance, whether the control store answered, job counts per state (the ten
//! frozen states, always all present), and data-volume and store sizes for the capacity
//! model. Nothing tenant-, principal- or credential-derived appears: no labels other
//! than the fixed job-state names, no paths, no identifiers. The control read is bounded
//! (2 s); when it cannot complete, `airs_mcp_control_ok` is 0 and the job series read 0.
//!
//! The filesystem sizes are bounded in time too (M8 review 1 minor): they come from a
//! per-store cache refreshed on the blocking pool. A scrape waits at most
//! [`FS_BUDGET`] for a refresh; a stalled refresh is never started twice; and the
//! scrape reports how old its filesystem sample is and whether one is available.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use crate::jobs::State;
use crate::tools::Storage;

/// The control read budget of one scrape.
pub const SCRAPE_BUDGET: Duration = Duration::from_secs(2);

/// At most this many directory entries are summed per store (bounded scrape).
const MAX_ENTRIES: usize = 4096;

/// How long one scrape waits for a filesystem refresh.
pub const FS_BUDGET: Duration = Duration::from_millis(500);

/// A filesystem sample younger than this is served without a refresh.
pub const FS_MAX_AGE: Duration = Duration::from_secs(30);

/// The filesystem sizes of one store.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FsSizes {
    /// `(size, free)` bytes of the data volume, when `statvfs` answered.
    pub volume: Option<(u64, u64)>,
    pub archive: u64,
    pub control: u64,
}

#[derive(Default)]
struct FsEntry {
    last: Option<(Instant, FsSizes)>,
    in_flight: bool,
}

/// The time-bounded filesystem sample cache: one entry per store.
#[derive(Default)]
pub struct FsCache {
    entries: Mutex<HashMap<PathBuf, FsEntry>>,
}

impl FsCache {
    /// The newest sample of `key` and its age, refreshing it with `sampler` on the
    /// blocking pool when it is older than [`FS_MAX_AGE`]. Waits at most `budget` for the
    /// refresh; one that has not finished keeps running (and is never started twice)
    /// while the scrape answers with what it has: possibly nothing.
    pub async fn get(
        self: &Arc<Self>,
        key: &Path,
        sampler: impl FnOnce() -> FsSizes + Send + 'static,
        budget: Duration,
    ) -> Option<(FsSizes, Duration)> {
        let fresh = |e: &FsEntry| e.last.is_some_and(|(at, _)| at.elapsed() < FS_MAX_AGE);
        {
            let mut map = self
                .entries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let e = map.entry(key.to_path_buf()).or_default();
            if !fresh(e) && !e.in_flight {
                e.in_flight = true;
                let me = Arc::clone(self);
                let k = key.to_path_buf();
                tokio::task::spawn_blocking(move || {
                    let sizes = sampler();
                    let mut map = me
                        .entries
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    let e = map.entry(k).or_default();
                    e.last = Some((Instant::now(), sizes));
                    e.in_flight = false;
                });
            }
        }
        let deadline = Instant::now() + budget;
        loop {
            {
                let map = self
                    .entries
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let e = map.get(key);
                let done = e.is_none_or(|e| !e.in_flight);
                if done || Instant::now() >= deadline {
                    return e.and_then(|e| e.last).map(|(at, s)| (s, at.elapsed()));
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

static FS: LazyLock<Arc<FsCache>> = LazyLock::new(Arc::default);

/// What one scrape observed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sample {
    pub ready: bool,
    pub maintenance: bool,
    /// Job counts per state, `None` when the control store did not answer in budget.
    pub jobs: Option<Vec<(State, u64)>>,
    /// `(size, free)` bytes of the data volume, when `statvfs` answered.
    pub volume: Option<(u64, u64)>,
    pub archive_bytes: u64,
    pub control_bytes: u64,
    /// Whether a filesystem sample exists, and its age in whole seconds.
    pub storage_available: bool,
    pub storage_age_seconds: u64,
    /// The data volume against the disk reserve (contract §9.5), when measured within
    /// [`FS_BUDGET`].
    pub space: Option<crate::space::Space>,
}

/// Sum the sizes of regular files directly in `dir` and one level below (archive roots
/// are `history/<cloud>/archive.sqlite3*`), bounded by [`MAX_ENTRIES`].
pub fn store_bytes(dir: &Path) -> u64 {
    let mut total = 0u64;
    let mut seen = 0usize;
    let mut stack = vec![(dir.to_path_buf(), 0u8)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            seen += 1;
            if seen > MAX_ENTRIES {
                return total;
            }
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_file() {
                total = total.saturating_add(meta.len());
            } else if meta.is_dir() && depth == 0 {
                stack.push((e.path(), 1));
            }
        }
    }
    total
}

/// [`store_bytes`] for the disk reserve, where an error must not read as zero. An
/// absent `dir` is 0 bytes. A directory that cannot be listed, an entry that cannot be
/// inspected, or more than `MAX_ENTRIES` entries make it unmeasurable: `None`, which
/// the reserve treats as `unknown` (M8 review 2 finding 7).
pub fn store_bytes_checked(dir: &Path) -> Option<u64> {
    let mut total = 0u64;
    let mut seen = 0usize;
    let mut stack = vec![(dir.to_path_buf(), 0u8)];
    while let Some((d, depth)) = stack.pop() {
        let entries = match std::fs::read_dir(&d) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && depth == 0 => return Some(0),
            Err(_) => return None,
        };
        for e in entries {
            let e = e.ok()?;
            seen += 1;
            if seen > MAX_ENTRIES {
                return None;
            }
            let meta = e.metadata().ok()?;
            if meta.is_file() {
                total = total.saturating_add(meta.len());
            } else if meta.is_dir() && depth == 0 {
                stack.push((e.path(), 1));
            }
        }
    }
    Some(total)
}

/// `(size, free)` bytes of the filesystem holding `dir`.
pub fn volume_bytes(dir: &Path) -> Option<(u64, u64)> {
    let s = rustix::fs::statvfs(dir).ok()?;
    let frag = s.f_frsize;
    Some((
        s.f_blocks.saturating_mul(frag),
        s.f_bavail.saturating_mul(frag),
    ))
}

/// Take one sample: the bounded control read happens off the async runtime.
pub async fn sample(
    storage: Option<&Arc<Storage>>,
    tenant: &str,
    ready: bool,
    maintenance: bool,
) -> Sample {
    let mut out = Sample {
        ready,
        maintenance,
        ..Sample::default()
    };
    let Some(storage) = storage else {
        return out;
    };
    out.jobs = storage
        .control_bounded(SCRAPE_BUDGET, |c| Ok(c.job_state_counts()?))
        .await
        .ok();
    let (data, history) = (storage.admitted.path.clone(), storage.history_root.clone());
    let key = data.clone();
    let sampler = move || FsSizes {
        volume: volume_bytes(&data),
        archive: store_bytes(&history),
        control: store_bytes(&data.join("control")),
    };
    if let Some((sizes, age)) = FS.get(&key, sampler, FS_BUDGET).await {
        out.volume = sizes.volume;
        out.archive_bytes = sizes.archive;
        out.control_bytes = sizes.control;
        out.storage_available = true;
        out.storage_age_seconds = age.as_secs();
    }
    // The shared reserve sampler: one measurement in flight at most, a bounded wait,
    // and `Unknown` (writes paused, reported as such) when it does not answer in time
    // (M8 review 2 finding 8).
    out.space = Some(storage.space_sampled(tenant).await.0);
    out
}

/// Render a sample as Prometheus text (fixed series, deterministic order).
pub fn render(s: &Sample) -> String {
    let mut o = String::new();
    let gauge = |o: &mut String, name: &str, help: &str, v: u64| {
        o.push_str(&format!(
            "# HELP {name} {help}\n# TYPE {name} gauge\n{name} {v}\n"
        ));
    };
    gauge(&mut o, "airs_mcp_up", "The service process is serving.", 1);
    gauge(
        &mut o,
        "airs_mcp_ready",
        "Ordinary operation is available (not in maintenance).",
        u64::from(s.ready && !s.maintenance),
    );
    gauge(
        &mut o,
        "airs_mcp_maintenance",
        "The service is in maintenance mode.",
        u64::from(s.maintenance),
    );
    gauge(
        &mut o,
        "airs_mcp_control_ok",
        "The control store answered within the scrape budget.",
        u64::from(s.jobs.is_some()),
    );
    o.push_str("# HELP airs_mcp_jobs Jobs per state.\n# TYPE airs_mcp_jobs gauge\n");
    for state in State::ALL {
        let n = s
            .jobs
            .as_ref()
            .and_then(|j| j.iter().find(|(st, _)| *st == state).map(|(_, n)| *n))
            .unwrap_or(0);
        o.push_str(&format!(
            "airs_mcp_jobs{{state=\"{}\"}} {n}\n",
            state.name()
        ));
    }
    let (size, free) = s.volume.unwrap_or((0, 0));
    gauge(
        &mut o,
        "airs_mcp_data_volume_size_bytes",
        "Size of the data volume.",
        size,
    );
    gauge(
        &mut o,
        "airs_mcp_data_volume_free_bytes",
        "Free bytes available to the service on the data volume.",
        free,
    );
    gauge(
        &mut o,
        "airs_mcp_archive_bytes",
        "Bytes of the history archive files.",
        s.archive_bytes,
    );
    gauge(
        &mut o,
        "airs_mcp_control_bytes",
        "Bytes of the control store files.",
        s.control_bytes,
    );
    gauge(
        &mut o,
        "airs_mcp_storage_sample_available",
        "A filesystem size sample is available (sizes read 0 when it is not).",
        u64::from(s.storage_available),
    );
    gauge(
        &mut o,
        "airs_mcp_storage_sample_age_seconds",
        "Age of the filesystem size sample.",
        s.storage_age_seconds,
    );
    gauge(
        &mut o,
        "airs_mcp_storage_reserve_available",
        "The disk reserve was measured (the reserve series read 0 when it was not; writes then pause).",
        u64::from(
            s.space
                .is_some_and(|sp| sp.level != crate::space::Level::Unknown),
        ),
    );
    gauge(
        &mut o,
        "airs_mcp_storage_reserve_bytes",
        "The disk reserve: 2x store bytes + 128 MiB + upload reservations + 64 MiB.",
        s.space.map_or(0, |sp| sp.reserve),
    );
    // An unmeasured reserve pauses every allocating write (fail closed), and reports so.
    let paused = |level| {
        s.space
            .is_some_and(|sp| sp.level == level || sp.level == crate::space::Level::Unknown)
    };
    o.push_str("# HELP airs_mcp_storage_low_space Writes paused by the disk reserve (publication below reserve + 512 MiB, collection below the reserve).\n# TYPE airs_mcp_storage_low_space gauge\n");
    o.push_str(&format!(
        "airs_mcp_storage_low_space{{level=\"publication\"}} {}\n",
        u64::from(
            paused(crate::space::Level::PublicationPaused)
                || paused(crate::space::Level::CollectionPaused)
        )
    ));
    o.push_str(&format!(
        "airs_mcp_storage_low_space{{level=\"collection\"}} {}\n",
        u64::from(paused(crate::space::Level::CollectionPaused))
    ));
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_series_is_fixed_and_jobs_cover_all_states() {
        let text = render(&Sample {
            ready: true,
            maintenance: false,
            jobs: Some(vec![(State::Queued, 3), (State::Failed, 1)]),
            volume: Some((1000, 400)),
            archive_bytes: 7,
            control_bytes: 9,
            storage_available: true,
            storage_age_seconds: 3,
            space: Some(crate::space::Space {
                free: Some(100),
                reserve: 200,
                level: crate::space::Level::CollectionPaused,
            }),
        });
        assert!(text.contains("airs_mcp_storage_reserve_available 1\n"));
        assert!(text.contains("airs_mcp_storage_reserve_bytes 200\n"));
        assert!(text.contains("airs_mcp_storage_low_space{level=\"publication\"} 1\n"));
        assert!(text.contains("airs_mcp_storage_low_space{level=\"collection\"} 1\n"));
        assert!(text.contains("airs_mcp_storage_sample_available 1\n"));
        assert!(text.contains("airs_mcp_storage_sample_age_seconds 3\n"));
        assert!(text.contains("airs_mcp_ready 1\n"));
        assert!(text.contains("airs_mcp_control_ok 1\n"));
        assert!(text.contains("airs_mcp_jobs{state=\"queued\"} 3\n"));
        assert!(text.contains("airs_mcp_jobs{state=\"failed\"} 1\n"));
        assert!(text.contains("airs_mcp_jobs{state=\"running\"} 0\n"));
        assert_eq!(text.matches("airs_mcp_jobs{").count(), 10);
        assert!(text.contains("airs_mcp_data_volume_free_bytes 400\n"));
        // Labels are only the fixed job-state names and the two fixed reserve levels.
        for line in text.lines().filter(|l| !l.starts_with('#')) {
            if let Some(i) = line.find('{') {
                assert!(
                    line[i..].starts_with("{state=\"")
                        || line[i..].starts_with("{level=\"publication\"}")
                        || line[i..].starts_with("{level=\"collection\"}"),
                    "{line}"
                );
            }
        }
    }

    #[test]
    fn an_unanswered_control_store_reports_zero_and_not_ok() {
        let text = render(&Sample {
            ready: true,
            maintenance: true,
            jobs: None,
            ..Sample::default()
        });
        assert!(text.contains("airs_mcp_ready 0\n"));
        assert!(text.contains("airs_mcp_maintenance 1\n"));
        assert!(text.contains("airs_mcp_control_ok 0\n"));
        assert!(text.contains("airs_mcp_jobs{state=\"queued\"} 0\n"));
    }

    /// A stalled filesystem sampler never holds a scrape past its budget, is never
    /// started twice, and its result is served once it lands.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_stalled_filesystem_sample_does_not_block_a_scrape() {
        let cache = Arc::new(FsCache::default());
        let key = Path::new("/synthetic/store");
        let runs = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let release = Arc::new(std::sync::Barrier::new(2));
        let sampler = |runs: Arc<std::sync::atomic::AtomicUsize>,
                       release: Arc<std::sync::Barrier>| {
            move || {
                runs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                release.wait();
                FsSizes {
                    volume: Some((10, 4)),
                    archive: 1,
                    control: 2,
                }
            }
        };
        let budget = Duration::from_millis(100);
        let started = Instant::now();
        let first = cache
            .get(key, sampler(runs.clone(), release.clone()), budget)
            .await;
        assert!(
            started.elapsed() < Duration::from_millis(600),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(first, None, "nothing yet: the scrape reports unavailable");
        // A second scrape while the first refresh is stalled starts no second one.
        let started = Instant::now();
        let second = cache
            .get(key, sampler(runs.clone(), release.clone()), budget)
            .await;
        assert!(started.elapsed() < Duration::from_millis(600));
        assert_eq!(second, None);
        assert_eq!(runs.load(std::sync::atomic::Ordering::SeqCst), 1);
        // The stalled refresh finishes; the next scrape serves its result, fresh.
        release.wait();
        let third = cache
            .get(key, sampler(runs.clone(), release.clone()), budget)
            .await;
        let (sizes, age) = third.expect("the landed sample");
        assert_eq!(sizes.volume, Some((10, 4)));
        assert!(age < FS_MAX_AGE);
        assert_eq!(
            runs.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "fresh: no refresh"
        );
    }

    #[test]
    fn store_sizes_are_bounded_sums_of_regular_files() {
        let d = std::env::temp_dir().join(format!("airs-mcp-metrics-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(d.join("cloud")).unwrap();
        std::fs::write(d.join("a"), [0u8; 10]).unwrap();
        std::fs::write(d.join("cloud").join("b"), [0u8; 5]).unwrap();
        std::fs::create_dir_all(d.join("cloud").join("deep")).unwrap();
        std::fs::write(d.join("cloud").join("deep").join("c"), [0u8; 100]).unwrap();
        assert_eq!(store_bytes(&d), 15, "two levels only");
        assert!(volume_bytes(&d).is_some_and(|(size, free)| size >= free));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
