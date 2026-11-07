//! Operational metrics on the private health listener (plan M8.1; contract §9.5).
//!
//! `GET /metrics` returns Prometheus text with a fixed set of series: readiness,
//! maintenance, whether the control store answered, job counts per state (the ten
//! frozen states, always all present), and data-volume and store sizes for the capacity
//! model. Nothing tenant-, principal- or credential-derived appears: no labels other
//! than the fixed job-state names, no paths, no identifiers. The control read is bounded
//! (2 s); when it cannot complete, `airs_mcp_control_ok` is 0 and the job series read 0.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::jobs::State;
use crate::tools::Storage;

/// The control read budget of one scrape.
pub const SCRAPE_BUDGET: Duration = Duration::from_secs(2);

/// At most this many directory entries are summed per store (bounded scrape).
const MAX_ENTRIES: usize = 4096;

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
pub async fn sample(storage: Option<&Arc<Storage>>, ready: bool, maintenance: bool) -> Sample {
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
    let sizes = tokio::task::spawn_blocking(move || {
        (
            volume_bytes(&data),
            store_bytes(&history),
            store_bytes(&data.join("control")),
        )
    })
    .await
    .ok();
    if let Some((volume, archive, control)) = sizes {
        out.volume = volume;
        out.archive_bytes = archive;
        out.control_bytes = control;
    }
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
        });
        assert!(text.contains("airs_mcp_ready 1\n"));
        assert!(text.contains("airs_mcp_control_ok 1\n"));
        assert!(text.contains("airs_mcp_jobs{state=\"queued\"} 3\n"));
        assert!(text.contains("airs_mcp_jobs{state=\"failed\"} 1\n"));
        assert!(text.contains("airs_mcp_jobs{state=\"running\"} 0\n"));
        assert_eq!(text.matches("airs_mcp_jobs{").count(), 10);
        assert!(text.contains("airs_mcp_data_volume_free_bytes 400\n"));
        // Labels are only the fixed job-state names.
        for line in text.lines().filter(|l| !l.starts_with('#')) {
            if let Some(i) = line.find('{') {
                assert!(line[i..].starts_with("{state=\""), "{line}");
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
