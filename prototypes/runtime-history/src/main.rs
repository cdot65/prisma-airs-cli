//! Isolated B0 feasibility probe. No CLI integration or customer data.
#![forbid(unsafe_code)]

use rusqlite::{Connection, OpenFlags, params};
use serde_json::{Value, json};
use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SCHEMA: &str = include_str!("../schema.sql");
const DAY: i64 = 86_400_000;
const END: i64 = 1_794_009_600_000;
const HEAP_LIMIT: i64 = 256 * 1024 * 1024;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn require(ok: bool, what: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(what.into()) }
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Result<Self> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "airs-history-prototype-{}-{stamp}",
            std::process::id()
        ));
        fs::DirBuilder::new().mode(0o700).create(&dir)?;
        Ok(Self(dir))
    }
    fn db(&self) -> PathBuf {
        self.0.join("archive.sqlite3")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn writer(path: &Path, create: bool) -> Result<Connection> {
    if create {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?
            .sync_all()?;
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    // Callers install one shared operation deadline before any contended work.
    conn.busy_timeout(Duration::ZERO)?;
    conn.execute_batch(
        "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;
        PRAGMA foreign_keys=ON; PRAGMA secure_delete=ON; PRAGMA temp_store=MEMORY;
        PRAGMA mmap_size=0; PRAGMA cache_size=-8192; PRAGMA cache_spill=OFF;
        PRAGMA trusted_schema=OFF;",
    )?;
    set_limits(&conn)?;
    Ok(conn)
}

fn set_limits(conn: &Connection) -> Result<()> {
    use rusqlite::limits::Limit;
    for (limit, value) in [
        (Limit::SQLITE_LIMIT_LENGTH, 1024 * 1024),
        (Limit::SQLITE_LIMIT_SQL_LENGTH, 64 * 1024),
        (Limit::SQLITE_LIMIT_COLUMN, 128),
        (Limit::SQLITE_LIMIT_EXPR_DEPTH, 64),
        (Limit::SQLITE_LIMIT_COMPOUND_SELECT, 16),
        (Limit::SQLITE_LIMIT_ATTACHED, 0),
        (Limit::SQLITE_LIMIT_VARIABLE_NUMBER, 128),
        (Limit::SQLITE_LIMIT_TRIGGER_DEPTH, 16),
    ] {
        conn.set_limit(limit, value)?;
    }
    Ok(())
}

fn pause_before(deadline: Instant) -> bool {
    if Instant::now() >= deadline {
        return false;
    }
    std::thread::sleep(
        Duration::from_millis(1).min(deadline.saturating_duration_since(Instant::now())),
    );
    Instant::now() < deadline
}

fn set_deadline(conn: &mut Connection, deadline: Instant) -> Result<()> {
    conn.busy_handler(Some(move |_| pause_before(deadline)))?;
    conn.progress_handler(1000, Some(move || Instant::now() >= deadline))?;
    Ok(())
}

fn reader(path: &Path, budget: Duration) -> Result<Connection> {
    let deadline = Instant::now() + budget;
    let mut conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    set_deadline(&mut conn, deadline)?;
    conn.execute_batch(
        "PRAGMA query_only=ON; PRAGMA foreign_keys=ON;
        PRAGMA temp_store=MEMORY; PRAGMA mmap_size=0; PRAGMA cache_size=-8192;
        PRAGMA trusted_schema=OFF;",
    )?;
    set_limits(&conn)?;
    let mode: String = conn.query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
    require(mode == "delete", "reader must not repair a journal mode")?;
    Ok(conn)
}

fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    conn.execute(
        "INSERT INTO archive_meta VALUES (1,'synthetic-cloud','123','env1:synthetic','[]',?1,1,1,?2)",
        params![END, END - 1095 * DAY],
    )?;
    conn.execute(
        "INSERT INTO collection_runs VALUES ('synthetic-run','sync',?1,?1,?2,?1,'partial','{}')",
        params![END, END - 1095 * DAY],
    )?;
    Ok(())
}

fn schema_invariants() -> Result<()> {
    let f = Fixture::new()?;
    let conn = writer(&f.db(), true)?;
    init(&conn)?;
    require(conn.execute("INSERT INTO receipts VALUES ('bad','bad',0,0,NULL,NULL,NULL,NULL,NULL,NULL,'{}','promoted',NULL)", []).is_err(), "schema admitted a promoted orphan")?;
    conn.execute_batch("INSERT INTO events VALUES ('event','app','App','session','scan',0,NULL,1,1,1,NULL,'unknown','unknown',0);
        INSERT INTO receipts VALUES ('receipt','receipt',1,1,'scan','app','App','session',0,NULL,'{}','promoted','event');")?;
    require(
        conn.execute("DELETE FROM events", []).is_err(),
        "event deletion orphaned a receipt",
    )?;
    conn.execute_batch("BEGIN IMMEDIATE;
        UPDATE receipts SET state='partial',promoted_event_id=NULL WHERE receipt_id='receipt';
        DELETE FROM events; COMMIT;
        INSERT INTO sync_work(work_id,kind,descriptor_json,state,binding_revision,retry_epoch,attempts,provenance_json)
            VALUES ('work','reports','{}','pending','revision',0,0,'{}');
        INSERT INTO work_attempts VALUES ('first','work','reports','synthetic-run','transport','revision',0,1,1,2,403,'auth_denied','{}');
        INSERT INTO work_transitions VALUES ('reset','work',3,'retry_reset',1,'revision','revision','operator reset','{}');
        UPDATE sync_work SET retry_epoch=1,attempts=0 WHERE work_id='work';
        INSERT INTO work_attempts VALUES ('second','work','reports','synthetic-run','transport2','revision',1,1,4,5,NULL,'not_sent','{}');
        INSERT INTO work_attempts VALUES ('third','work','reports','synthetic-run','transport3','revision',1,1,6,7,200,'returned','{}');")?;
    let prior: i64 = conn.query_row(
        "SELECT count(*) FROM work_attempts WHERE outcome='auth_denied'",
        [],
        |r| r.get(0),
    )?;
    require(prior == 1, "retry reset erased authorization history")?;
    require(conn.execute("INSERT INTO work_attempts VALUES ('duplicate','work','reports','synthetic-run','transport4','revision',1,1,8,9,200,'returned','{}')", []).is_err(), "same retry ordinal admitted twice")?;
    require(conn.execute("INSERT INTO sync_work(work_id,kind,descriptor_json,state,binding_revision,attempts,provenance_json) VALUES ('bad-session','session','{}','pending','scanner',3,'{}')", []).is_err(), "session traversal inherited scanner retry limits")?;
    conn.execute("INSERT INTO sync_work(work_id,kind,descriptor_json,state,attempts,provenance_json) VALUES ('session','session','{}','pending',0,'{}')", [])?;
    require(conn.execute("INSERT INTO work_attempts VALUES ('bad-source','session','reports','synthetic-run','transport5','revision',0,1,8,9,200,'returned','{}')", []).is_err(), "scanner attempt attached to management work")?;
    for i in 0..6 {
        conn.execute("INSERT INTO management_reads VALUES (?1,'synthetic-run','sessions',NULL,NULL,1,2,200,1,25,'returned','{}')", [format!("page-{i}")])?;
    }
    Ok(())
}

fn signature(dir: &Path) -> Result<Value> {
    let mut rows = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let m = entry.metadata()?;
        rows.push(
            json!({"name":entry.file_name().to_string_lossy(), "mode":m.mode(), "len":m.len(),
            "modified":m.modified()?.duration_since(UNIX_EPOCH)?.as_nanos().to_string(),
            "bytes":fs::read(entry.path())?}),
        );
    }
    rows.sort_by_key(|v| v["name"].as_str().unwrap_or("").to_owned());
    Ok(json!(rows))
}

fn qualify() -> Result<Value> {
    schema_invariants()?;
    let f = Fixture::new()?;
    let db = f.db();
    let mut conn = writer(&db, true)?;
    init(&conn)?;
    let version: String = conn.query_row("SELECT sqlite_version()", [], |r| r.get(0))?;
    let source_id: String = conn.query_row("SELECT sqlite_source_id()", [], |r| r.get(0))?;
    let extension = conn.query_row(
        "SELECT load_extension('B0-never-load-an-extension')",
        [],
        |r| r.get::<_, i64>(0),
    );
    require(
        matches!(extension, Err(rusqlite::Error::SqliteFailure(_, Some(ref message))) if message == "not authorized"),
        "SQLite extension loading was not disabled",
    )?;
    let compile_options = conn
        .prepare("PRAGMA compile_options")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    conn.execute_batch(
        "CREATE TABLE canaries (id INTEGER PRIMARY KEY, text TEXT NOT NULL) STRICT;",
    )?;
    let marker = "B0_SECURE_DELETE_SYNTHETIC_MARKER_7c8c9739".repeat(100);
    conn.execute("INSERT INTO canaries(text) VALUES (?1)", [&marker])?;
    require(
        fs::read(&db)?
            .windows(32)
            .any(|s| s == &marker.as_bytes()[..32]),
        "canary must actually reach disk",
    )?;
    conn.execute("DELETE FROM canaries", [])?;
    require(
        !fs::read(&db)?
            .windows(32)
            .any(|s| s == &marker.as_bytes()[..32]),
        "secure_delete left marker in live database",
    )?;
    require(
        !db.with_extension("sqlite3-journal").exists(),
        "DELETE left a journal after commit",
    )?;
    conn.execute_batch("BEGIN IMMEDIATE; INSERT INTO canaries(text) VALUES ('uncommitted')")?;
    let journal = db.with_extension("sqlite3-journal");
    require(
        fs::metadata(&journal)?.mode() & 0o777 == 0o600,
        "journal was not created privately",
    )?;
    let during = signature(&f.0)?;
    let read = reader(&db, Duration::from_secs(2))?;
    let count: i64 = read.query_row("SELECT count(*) FROM canaries", [], |r| r.get(0))?;
    require(count == 0, "reader observed an uncommitted write")?;
    drop(read);
    require(
        signature(&f.0)? == during,
        "read changed an active writer's journal",
    )?;
    conn.execute_batch("ROLLBACK")?;

    let read = reader(&db, Duration::from_secs(2))?;
    read.execute_batch("BEGIN DEFERRED")?;
    let first: i64 = read.query_row("SELECT count(*) FROM canaries", [], |r| r.get(0))?;
    conn.execute_batch("BEGIN IMMEDIATE; INSERT INTO canaries(text) VALUES ('later')")?;
    require(
        conn.execute_batch("COMMIT").is_err(),
        "writer crossed a coherent read transaction",
    )?;
    let second: i64 = read.query_row("SELECT count(*) FROM canaries", [], |r| r.get(0))?;
    require(
        first == 0 && second == first,
        "two-query reader mixed observations",
    )?;
    read.execute_batch("COMMIT")?;
    conn.execute_batch("COMMIT; DELETE FROM canaries")?;
    drop(read);
    let before = signature(&f.0)?;
    let read = reader(&db, Duration::from_secs(2))?;
    let count: i64 = read.query_row("SELECT count(*) FROM archive_meta", [], |r| r.get(0))?;
    require(count == 1, "reader saw wrong metadata")?;
    drop(read);
    require(
        signature(&f.0)? == before,
        "read-only connection changed archive files",
    )?;
    let absent = f.0.join("missing.sqlite3");
    require(
        reader(&absent, Duration::from_secs(2)).is_err() && !absent.exists(),
        "reader created missing database",
    )?;

    let read = reader(&db, Duration::from_millis(100))?;
    let start = Instant::now();
    let interrupted = read.query_row("WITH RECURSIVE seq(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM seq WHERE n<1000000000) SELECT sum(n) FROM seq", [], |r| r.get::<_, i64>(0));
    let deadline_ms = start.elapsed().as_millis();
    require(
        matches!(interrupted, Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::OperationInterrupted),
        "deadline did not interrupt query",
    )?;
    require(
        deadline_ms < 150,
        "100ms query deadline exceeded 50ms scheduling allowance",
    )?;
    drop(read);

    let blocker = writer(&db, false)?;
    blocker.execute_batch("BEGIN EXCLUSIVE")?;
    use rustix::fs::{FlockOperation, flock};
    let contention_path = f.0.join("contention.lock");
    let gate_holder = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&contention_path)?;
    let gate_waiter = OpenOptions::new().read(true).open(&contention_path)?;
    flock(&gate_holder, FlockOperation::NonBlockingLockExclusive)?;
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        flock(&gate_holder, FlockOperation::Unlock).unwrap();
    });
    let start = Instant::now();
    let deadline = start + Duration::from_millis(250);
    loop {
        match flock(&gate_waiter, FlockOperation::NonBlockingLockShared) {
            Ok(()) => break,
            Err(rustix::io::Errno::WOULDBLOCK | rustix::io::Errno::INTR) => {
                require(
                    pause_before(deadline),
                    "gate consumed the entire contention budget",
                )?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    let gate_wait_ms = start.elapsed().as_millis();
    release.join().map_err(|_| "gate release thread failed")?;
    require(
        (75..200).contains(&gate_wait_ms),
        "probe did not exercise prior gate contention",
    )?;
    // Handoff the original deadline, never a fresh 250ms SQLite timeout.
    set_deadline(&mut conn, deadline)?;
    let sqlite_start = Instant::now();
    let busy = conn.execute_batch("BEGIN IMMEDIATE");
    let sqlite_wait_ms = sqlite_start.elapsed().as_millis();
    let busy_ms = start.elapsed().as_millis();
    require(
        matches!(busy, Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::DatabaseBusy),
        "contention did not return busy",
    )?;
    require(
        (240..300).contains(&busy_ms) && sqlite_wait_ms < 200,
        "cumulative 250ms gate+SQLite deadline exceeded 50ms scheduling allowance",
    )?;
    flock(&gate_waiter, FlockOperation::Unlock)?;
    let start = Instant::now();
    require(
        reader(&db, Duration::from_millis(100)).is_err(),
        "reader crossed exclusive writer",
    )?;
    let read_busy_ms = start.elapsed().as_millis();
    require(
        read_busy_ms < 150,
        "100ms reader lock deadline exceeded 50ms scheduling allowance",
    )?;
    blocker.execute_batch("ROLLBACK")?;
    // The old contention deadline is spent; remaining independent probes fail fast.
    conn.busy_timeout(Duration::ZERO)?;
    conn.progress_handler(0, None::<fn() -> bool>)?;

    let gate_path = f.0.join("maintenance.lock");
    let shared_gate = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&gate_path)?;
    let exclusive_gate = OpenOptions::new().read(true).write(true).open(&gate_path)?;
    flock(&shared_gate, FlockOperation::NonBlockingLockShared)?;
    require(
        flock(&exclusive_gate, FlockOperation::NonBlockingLockExclusive).is_err(),
        "maintenance crossed ordinary reader gate",
    )?;
    flock(&shared_gate, FlockOperation::Unlock)?;
    flock(&exclusive_gate, FlockOperation::NonBlockingLockExclusive)?;
    require(
        flock(&shared_gate, FlockOperation::NonBlockingLockShared).is_err(),
        "ordinary reader crossed maintenance gate",
    )?;
    flock(&exclusive_gate, FlockOperation::Unlock)?;
    let copy = f.0.join("compact.sqlite3");
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&copy)?;
    // VACUUM INTO internally attaches its destination. Only this fixed maintenance
    // statement may temporarily use one attachment; ordinary connections allow zero.
    conn.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_ATTACHED, 1)?;
    conn.execute("VACUUM INTO ?1", [copy.to_str().ok_or("non-UTF8 fixture")?])?;
    conn.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_ATTACHED, 0)?;
    File::open(&copy)?.sync_all()?;
    File::open(&f.0)?.sync_all()?;
    let copy_conn = reader(&copy, Duration::from_secs(2))?;
    let integrity: String = copy_conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    require(integrity == "ok", "compacted copy is not valid")?;
    require(
        fs::metadata(&copy)?.permissions().mode() & 0o777 == 0o600,
        "VACUUM changed private copy permissions",
    )?;
    require(
        fs::metadata(&f.0)?.permissions().mode() & 0o777 == 0o700,
        "fixture is not private",
    )?;
    Ok(
        json!({"sqlite_version":version,"sqlite_source_id":source_id,"compile_options":compile_options,
        "secure_delete_marker_absent":true,"read_only_unchanged":true,"missing_reader_no_create":true,
        "deadline_ms":deadline_ms,"cumulative_busy_ms":busy_ms,"gate_wait_ms":gate_wait_ms,
        "sqlite_wait_ms":sqlite_wait_ms,"scheduler_allowance_ms":50,"read_busy_ms":read_busy_ms,
        "private_active_journal_unchanged":true,"coherent_two_query_read":true,
        "maintenance_gate_excludes_readers":true,"vacuum_private_copy_valid":true,
        "receipt_and_attempt_schema_invariants":true,"extension_loading_disabled":true,
        "sqlite_heap_limit_bytes":HEAP_LIMIT}),
    )
}

fn benchmark(rows: usize, output: &Path) -> Result<Value> {
    require(rows > 0, "rows must be positive")?;
    let mut conn = writer(output, true)?;
    init(&conn)?;
    let start = Instant::now();
    let mut max_hold = Duration::ZERO;
    let mut hold_ms = Vec::new();
    let mut duplicate_facts = 0;
    let mut extra_facts = 0;
    for base in (0..rows).step_by(200) {
        let held = Instant::now();
        let tx = conn.transaction()?;
        {
            let mut event = tx.prepare(
                "INSERT INTO events VALUES (?1,?2,?3,?4,?5,0,NULL,?6,?7,?7,'profile',?8,?9,?10)",
            )?;
            let mut fact = tx.prepare(
                "INSERT OR IGNORE INTO event_facts VALUES (?1,?2,'session_detail',?3,1,?4)",
            )?;
            let mut link =
                tx.prepare("INSERT OR IGNORE INTO fact_runs VALUES (?1,'synthetic-run',?2)")?;
            let mut detector = tx.prepare("INSERT INTO detector_observations VALUES (?1,'prompt_injection','prompt','malicious',?2,0,'{}')")?;
            for i in base..(base + 200).min(rows) {
                let id = format!("event-{i:08}");
                let scan = format!("00000000-0000-4000-8000-{i:012}");
                let app = format!("app-{:02}", i % 16);
                let ms = END - ((i as i64 * 1095 * DAY) / rows as i64) - 1;
                let conflict = i % 1000 == 0;
                let malicious = i % 10 == 0;
                let verdict = if conflict {
                    "unknown"
                } else if malicious {
                    "malicious"
                } else {
                    "benign"
                };
                let action = if conflict {
                    "unknown"
                } else if i % 20 == 0 {
                    "block"
                } else {
                    "allow"
                };
                event.execute(params![
                    id,
                    app,
                    format!("App {}", i % 16),
                    format!("session-{}", i / 10),
                    scan,
                    ms,
                    END,
                    verdict,
                    action,
                    i64::from(conflict)
                ])?;
                let fact_id = format!("fact-{i:08}");
                let original = if malicious && !conflict {
                    "{\"verdict\":\"malicious\"}"
                } else {
                    "{\"verdict\":\"benign\"}"
                };
                fact.execute(params![fact_id, id, END, original])?;
                link.execute(params![fact_id, END])?;
                duplicate_facts +=
                    usize::from(fact.execute(params![fact_id, id, END + 1, original])? == 0);
                if conflict {
                    fact.execute(params![
                        format!("conflict-{i:08}"),
                        id,
                        END + 1,
                        "{\"verdict\":\"malicious\"}"
                    ])?;
                    extra_facts += 1;
                }
                if malicious {
                    detector.execute(params![id, action])?;
                }
            }
        }
        tx.commit()?;
        let elapsed = held.elapsed();
        max_hold = max_hold.max(elapsed);
        hold_ms.push(elapsed.as_secs_f64() * 1000.0);
    }
    conn.execute("INSERT INTO receipts VALUES ('partial','bulk/job/item',?1,?1,NULL,NULL,NULL,NULL,NULL,NULL,'{}','partial',NULL)", [END])?;
    conn.execute("INSERT INTO coverage_segments VALUES ('partial','synthetic-run',NULL,NULL,'sessions',?1,?2,?2,'partial','{\"reason\":\"synthetic page budget\"}')", params![END-1095*DAY,END])?;
    conn.execute_batch("ANALYZE")?;
    let ingest_seconds = start.elapsed().as_secs_f64();
    let mut queries = Vec::new();
    for days in [30, 90] {
        // All summaries share this transaction and deadline. No mixing snapshots.
        let read_start = Instant::now();
        let mut read = reader(output, Duration::from_secs(2))?;
        let tx = read.transaction()?;
        let since = END - days * DAY;
        let counts: (i64,i64,i64,i64) = tx.query_row("SELECT count(*),count(DISTINCT scan_id),coalesce(sum(verdict='malicious'),0),coalesce(sum(action='block'),0) FROM events WHERE event_ms>=?1 AND event_ms<?2",params![since,END],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        let apps = tx.prepare("SELECT app_id,verdict,action,count(*) FROM events WHERE event_ms>=?1 AND event_ms<?2 GROUP BY app_id,verdict,action")?.query_map(params![since,END],|r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let detector_count: i64 = tx.query_row("SELECT count(*) FROM events e JOIN detector_observations d ON d.event_id=e.event_id WHERE e.event_ms>=?1 AND e.event_ms<?2 AND d.detector='prompt_injection'",params![since,END],|r|r.get(0))?;
        let partial: i64 = tx.query_row(
            "SELECT count(*) FROM receipts WHERE state!='promoted'",
            [],
            |r| r.get(0),
        )?;
        tx.commit()?;
        let elapsed = read_start.elapsed().as_secs_f64();
        require(
            elapsed < 2.0,
            "million-row coherent report read exceeded 2 seconds",
        )?;
        require(partial == 1, "partial receipt counted as full event")?;
        let expected = (0..rows)
            .filter(|i| END - ((*i as i64 * 1095 * DAY) / rows as i64) > since)
            .count() as i64;
        require(
            counts.0 == expected && counts.1 == expected,
            "window cardinality mismatch",
        )?;
        queries.push(json!({"days":days,"seconds":elapsed,"event_observations":counts.0,"distinct_scan_ids":counts.1,"malicious":counts.2,"blocked":counts.3,"app_groups":apps.len(),"detector_observations":detector_count,"partial_receipts":partial}));
    }
    let event_count: i64 = conn.query_row("SELECT count(*) FROM events", [], |r| r.get(0))?;
    let fact_count: i64 = conn.query_row("SELECT count(*) FROM event_facts", [], |r| r.get(0))?;
    require(
        event_count == rows as i64
            && fact_count == (rows + extra_facts) as i64
            && duplicate_facts == rows,
        "idempotent fixture counts mismatch",
    )?;
    let quick: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    require(quick == "ok", "benchmark quick_check failed")?;
    hold_ms.sort_by(f64::total_cmp);
    let p50 = hold_ms[hold_ms.len() / 2];
    let p95 = hold_ms[(hold_ms.len() - 1) * 95 / 100];
    let p99 = hold_ms[(hold_ms.len() - 1) * 99 / 100];
    Ok(
        json!({"rows":rows,"span_days":1095,"applications":16,"ingest_seconds":ingest_seconds,
        "events_per_second": rows as f64/ingest_seconds,"largest_200_record_transaction_ms":max_hold.as_secs_f64()*1000.0,
        "transaction_ms":{"p50":p50,"p95":p95,"p99":p99},
        "duplicate_facts_deduplicated":duplicate_facts,"retained_conflicting_facts":extra_facts,
        "database_bytes":fs::metadata(output)?.len(),"queries":queries,"quick_check":quick}),
    )
}

fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    // Process-wide SQLite allocator cap, set once in this standalone prototype.
    let cap = Connection::open_in_memory()?;
    cap.pragma_update(None, "hard_heap_limit", HEAP_LIMIT)?;
    match args.as_slice() {
        [] => println!("{}", serde_json::to_string_pretty(&qualify()?)?),
        [command, rows, path] if command == "benchmark" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&benchmark(rows.parse()?, Path::new(path))?)?
            );
        }
        _ => return Err("usage: airs-history-prototype [benchmark ROWS NEW_DATABASE_PATH]".into()),
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("prototype failure: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_safety_and_deadlines() {
        qualify().unwrap();
    }
    #[test]
    fn multi_year_fixture_has_exact_counts() {
        let f = Fixture::new().unwrap();
        let value = benchmark(2000, &f.db()).unwrap();
        assert_eq!(value["rows"], 2000);
        assert_eq!(value["retained_conflicting_facts"], 2);
        assert_eq!(value["duplicate_facts_deduplicated"], 2000);
    }
}
