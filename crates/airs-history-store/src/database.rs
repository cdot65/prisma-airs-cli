//! Fixed SQLite policy, private atomic initialization and cooperative lock ordering.
use super::{
    failure,
    identity::Cloud,
    paths::{DATABASE, Directory, GATE, WRITER},
};
use airs_runtime_core::error::CliError;
use rusqlite::{Connection, OpenFlags, params};
use rustix::fs::{FlockOperation, flock};
use std::{
    fs::File,
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const SCHEMA: &str = include_str!("schema.sql");
/// The accepted M4 schema (version 1): the current one without `pruned_examples`,
/// `archive_generation` and `import_operations`. A version-1 archive is read as is and upgraded in place, by
/// adding those two tables, when a writer first opens it (M5 review 2 finding 8).
pub const SCHEMA_V1: &str = include_str!("schema_v1.sql");
/// The oldest schema version this build recognizes.
pub const OLDEST_VERSION: i64 = 1;
pub const APPLICATION_ID: i64 = 1095324243;
pub const VERSION: i64 = 2;
pub const READ_BUDGET: Duration = Duration::from_secs(2);
pub const RETAIN_BUDGET: Duration = Duration::from_millis(250);

fn cancelled(session: &scm::Session) -> Result<(), CliError> {
    if session.is_cancelled() {
        Err(CliError::Interrupted(
            airs_runtime_core::error::InterruptContext::NoDataWritten,
        ))
    } else {
        Ok(())
    }
}

pub fn sql_error(error: rusqlite::Error) -> CliError {
    use rusqlite::ErrorCode;
    let reason = match error {
        rusqlite::Error::SqliteFailure(code, _) => match code.code {
            ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => {
                "archive is busy; retry after the active operation"
            }
            ErrorCode::OperationInterrupted => {
                "archive operation exceeded its deadline or was interrupted"
            }
            ErrorCode::DiskFull => "archive storage is full; no successful retention is claimed",
            ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase => {
                "archive is corrupt; preserve it for explicit recovery"
            }
            ErrorCode::ReadOnly => "archive is read-only or needs explicit recovery",
            _ => "SQLite operation failed; archive was not recreated",
        },
        _ => "invalid archive data or unsupported schema",
    };
    failure(reason)
}

pub fn now_ms() -> Result<i64, CliError> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| failure("system clock predates the Unix epoch"))?
            .as_millis(),
    )
    .map_err(|_| failure("system time is outside the archive range"))
}

pub fn pause(deadline: Instant) -> bool {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return false;
    }
    std::thread::sleep(Duration::from_millis(1).min(remaining));
    Instant::now() < deadline
}

pub fn deadline(
    conn: &mut Connection,
    until: Instant,
    session: &scm::Session,
) -> Result<(), CliError> {
    let busy_session = session.clone();
    let progress_session = session.clone();
    conn.busy_handler(Some(move |_| !busy_session.is_cancelled() && pause(until)))
        .map_err(sql_error)?;
    conn.progress_handler(
        1000,
        Some(move || Instant::now() >= until || progress_session.is_cancelled()),
    )
    .map_err(sql_error)
}

fn heap_policy() -> Result<(), CliError> {
    // Set once for this CLI process. This limits SQLite allocations, not Rust/RSS.
    static RESULT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let ok = *RESULT.get_or_init(|| {
        let Ok(conn) = Connection::open_in_memory() else {
            return false;
        };
        conn.pragma_update(None, "hard_heap_limit", 256_i64 * 1024 * 1024)
            .is_ok()
            && conn
                .query_row("PRAGMA hard_heap_limit", [], |r| r.get::<_, i64>(0))
                .is_ok_and(|v| v > 0 && v <= 256 * 1024 * 1024)
    });
    if ok {
        Ok(())
    } else {
        Err(failure("cannot establish the SQLite allocator limit"))
    }
}

fn configure(conn: &Connection, write: bool) -> Result<(), CliError> {
    use rusqlite::limits::Limit;
    conn.execute_batch(
        "PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF;
        PRAGMA temp_store=MEMORY; PRAGMA cache_size=-8192; PRAGMA mmap_size=0;
        PRAGMA cache_spill=OFF;",
    )
    .map_err(sql_error)?;
    if write {
        conn.execute_batch(
            "PRAGMA synchronous=FULL; PRAGMA secure_delete=ON; PRAGMA fullfsync=ON;",
        )
        .map_err(sql_error)?;
    } else {
        conn.execute_batch("PRAGMA query_only=ON")
            .map_err(sql_error)?;
    }
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
        conn.set_limit(limit, value).map_err(sql_error)?;
    }
    Ok(())
}

fn check_header(file: &File) -> Result<(), CliError> {
    use std::os::unix::fs::FileExt;
    let mut header = [0; 100];
    let count = file
        .read_at(&mut header, 0)
        .map_err(|_| failure("cannot read archive header"))?;
    if count != 100 || &header[..16] != b"SQLite format 3\0" {
        return Err(failure(
            "archive is not a supported SQLite database; it was not recreated",
        ));
    }
    if header[18] != 1 || header[19] != 1 {
        return Err(failure(
            "WAL and unsupported journal formats are refused without changes",
        ));
    }
    let app = u32::from_be_bytes(
        header[68..72]
            .try_into()
            .map_err(|_| failure("invalid archive header"))?,
    );
    let version = u32::from_be_bytes(
        header[60..64]
            .try_into()
            .map_err(|_| failure("invalid archive header"))?,
    );
    if i64::from(app) != APPLICATION_ID {
        return Err(failure("unrecognized archive application identity"));
    }
    if i64::from(version) > VERSION {
        return Err(failure(
            "archive uses a newer schema; use a compatible airs version",
        ));
    }
    if i64::from(version) < OLDEST_VERSION {
        return Err(failure(
            "archive schema is unsupported; preserve it and use a compatible airs version",
        ));
    }
    Ok(())
}

pub fn no_sidecars(dir: &Directory, name: &str) -> Result<(), CliError> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let name = format!("{name}{suffix}");
        if dir.exists(&name)? {
            // Admit nothing to SQLite until path/ownership/hard-link checks pass.
            let _ = dir.file(&name, false, false)?;
            return Err(failure(if suffix == "-journal" {
                "an active or unfinished journal exists; retry after the writer, or use explicit recovery"
            } else {
                "WAL/SHM archives are unsupported and were left unchanged"
            }));
        }
    }
    Ok(())
}

fn schema_signature(conn: &Connection) -> Result<Vec<(String, String, String)>, CliError> {
    conn.prepare(
        "SELECT type,name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name",
    )
    .map_err(sql_error)?
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
    .map_err(sql_error)?
    .collect::<rusqlite::Result<Vec<_>>>()
    .map_err(sql_error)
}

fn validate(conn: &Connection, cloud: &Cloud) -> Result<(), CliError> {
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .map_err(sql_error)?;
    if mode != "delete" {
        return Err(failure(
            "archive journal mode is unsupported; it was left unchanged",
        ));
    }
    validate_content(conn, cloud)
}

/// The schema version a connection's database declares (`user_version`).
fn schema_version(conn: &Connection) -> Result<i64, CliError> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(sql_error)
}

/// One `CREATE TABLE <name> (...)` statement of the current schema, byte for byte (so
/// an upgraded archive has exactly the signature a new one has).
fn create_statement(name: &str) -> Result<&'static str, CliError> {
    let start = SCHEMA
        .find(&format!("CREATE TABLE {name} ("))
        .ok_or_else(|| failure("schema statement missing"))?;
    let end = SCHEMA[start..]
        .find(';')
        .ok_or_else(|| failure("schema statement unterminated"))?;
    Ok(&SCHEMA[start..=start + end])
}

/// Upgrade a version-1 (accepted M4) archive to the current schema in the caller's
/// connection: one exclusive transaction adds `pruned_examples` (no erasures yet) and
/// `archive_generation` (a fresh incarnation), and declares version 2. Anything other
/// than an exact version-1 archive was refused by validation before this runs.
pub(crate) fn upgrade_v1(conn: &Connection, cloud: &Cloud) -> Result<bool, CliError> {
    if schema_version(conn)? != 1 {
        return Ok(false);
    }
    conn.execute_batch("BEGIN EXCLUSIVE").map_err(sql_error)?;
    let result = upgrade_v1_in_transaction(conn, cloud);
    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT").map_err(sql_error)?;
            Ok(true)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// The version-1 → 2 upgrade inside the caller's transaction (a writer's own, or a
/// verified recovery's): `pruned_examples` (no erasures yet), `archive_generation` (the
/// first incarnation) and `import_operations`, then version 2, validated.
fn upgrade_v1_in_transaction(conn: &Connection, cloud: &Cloud) -> Result<(), CliError> {
    validate_content(conn, cloud)?;
    conn.execute_batch(create_statement("pruned_examples")?)
        .map_err(sql_error)?;
    conn.execute_batch(create_statement("archive_generation")?)
        .map_err(sql_error)?;
    conn.execute_batch(create_statement("import_operations")?)
        .map_err(sql_error)?;
    conn.execute(
        "INSERT INTO archive_generation VALUES (1, ?1)",
        [next_incarnation(0)?],
    )
    .map_err(sql_error)?;
    conn.execute_batch(&format!("PRAGMA user_version = {VERSION}"))
        .map_err(sql_error)?;
    validate_content(conn, cloud)?;
    integrity(conn)
}

/// The upgrade's backup protocol: a private copy of the version-1 database (taken under
/// the writer gate, so no writer changes it meanwhile) with its size and SHA-256, and a
/// manifest naming it, both durable before the upgrade begins. Copies are never deleted
/// here (an operator removes them once satisfied).
fn backup_before_upgrade(
    dir: &Directory,
    db: &File,
    cloud: &Cloud,
    until: Instant,
    session: &scm::Session,
) -> Result<(), CliError> {
    let length = db
        .metadata()
        .map_err(|_| failure("cannot inspect the archive before its upgrade"))?
        .len();
    space(dir, 2 * u128::from(length) + 1024 * 1024)?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    let copy = copy_backup(
        dir,
        &format!(".pre-upgrade-v1.{id}.sqlite3"),
        db,
        until,
        session,
    )?;
    manifest(
        dir,
        &format!(".pre-upgrade-v1.{id}.json"),
        &serde_json::json!({"format":"airs-runtime-history-pre-upgrade-backup","version":1,"from_schema_version":1,"to_schema_version":VERSION,"cloud_id":cloud.cloud_id,"sqlite_version":rusqlite::version(),"created_ms":now_ms()?,"database":copy}),
    )
}

/// The schema version an archive connection declares (`user_version`).
pub(crate) fn archive_schema_version(conn: &Connection) -> Result<i64, CliError> {
    schema_version(conn)
}

fn validate_content(conn: &Connection, cloud: &Cloud) -> Result<(), CliError> {
    let expected = Connection::open_in_memory().map_err(sql_error)?;
    expected
        .execute_batch(match schema_version(conn)? {
            1 => SCHEMA_V1,
            VERSION => SCHEMA,
            _ => return Err(failure("archive schema differs from the supported version")),
        })
        .map_err(sql_error)?;
    if schema_signature(conn)? != schema_signature(&expected)? {
        return Err(failure("archive schema differs from the supported version"));
    }
    let meta:(String,String,String,String,i64,i64)=conn.query_row("SELECT cloud_id,tsg_id,environment_id,environment_json,analysis_version,redaction_version FROM archive_meta WHERE singleton=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(sql_error)?;
    if meta
        != (
            cloud.cloud_id.clone(),
            cloud.tsg.clone(),
            cloud.environment_id.clone(),
            cloud.descriptor()?,
            1,
            1,
        )
    {
        return Err(failure(
            "archive cloud or analysis identity does not match the selected context",
        ));
    }
    Ok(())
}

pub struct Lock {
    file: File,
    name: String,
}
impl Lock {
    fn try_acquire(dir: &Directory, name: &str) -> Result<Self, CliError> {
        let file = dir.file(name, false, false)?;
        flock(&file, FlockOperation::NonBlockingLockExclusive)
            .map_err(|_| failure("archive already has an active sync or maintenance invocation"))?;
        dir.check_file(name, &file)?;
        Ok(Self {
            file,
            name: name.into(),
        })
    }
    pub fn acquire(
        dir: &Directory,
        name: &str,
        exclusive: bool,
        until: Instant,
        session: &scm::Session,
    ) -> Result<Self, CliError> {
        let file = dir.file(name, false, false)?;
        Self::acquire_opened(dir, name, file, exclusive, until, session)
    }
    fn acquire_opened(
        dir: &Directory,
        name: &str,
        file: File,
        exclusive: bool,
        until: Instant,
        session: &scm::Session,
    ) -> Result<Self, CliError> {
        loop {
            if session.is_cancelled() {
                return Err(CliError::Interrupted(
                    airs_runtime_core::error::InterruptContext::NoDataWritten,
                ));
            }
            if Instant::now() >= until {
                return Err(failure("archive lock deadline exceeded"));
            }
            match flock(
                &file,
                if exclusive {
                    FlockOperation::NonBlockingLockExclusive
                } else {
                    FlockOperation::NonBlockingLockShared
                },
            ) {
                Ok(()) => {
                    dir.check_file(name, &file)?;
                    return Ok(Self {
                        file,
                        name: name.to_owned(),
                    });
                }
                Err(rustix::io::Errno::WOULDBLOCK | rustix::io::Errno::INTR) => {
                    if !pause(until) {
                        return Err(failure("archive is busy; lock deadline exceeded"));
                    }
                }
                Err(_) => return Err(failure("cannot acquire archive lock")),
            }
        }
    }
}
impl Lock {
    pub fn check(&self, dir: &Directory) -> Result<(), CliError> {
        dir.check_file(&self.name, &self.file)
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = flock(&self.file, FlockOperation::Unlock);
    }
}

pub struct Reader {
    pub conn: Connection,
    pub dir: Directory,
    db: File,
    _gate: Lock,
    _lease: Option<Lock>,
    pub until: Instant,
    session: scm::Session,
}
impl Reader {
    pub fn open(
        root: &Path,
        cloud: &Cloud,
        session: &scm::Session,
    ) -> Result<Option<Self>, CliError> {
        Self::open_existing(root, cloud, READ_BUDGET, false, session)
    }
    /// A shared coherent read with its own budget (report rendering: one snapshot for
    /// the summary, the detail and the posture of a long period).
    pub fn within(
        root: &Path,
        cloud: &Cloud,
        budget: Duration,
        session: &scm::Session,
    ) -> Result<Option<Self>, CliError> {
        Self::open_existing(root, cloud, budget, false, session)
    }
    pub fn maintenance(
        root: &Path,
        cloud: &Cloud,
        budget: Duration,
        session: &scm::Session,
    ) -> Result<Option<Self>, CliError> {
        Self::open_existing(root, cloud, budget, true, session)
    }
    fn open_existing(
        root: &Path,
        cloud: &Cloud,
        budget: Duration,
        maintenance: bool,
        session: &scm::Session,
    ) -> Result<Option<Self>, CliError> {
        let until = Instant::now() + budget;
        cancelled(session)?;
        heap_policy()?;
        let Some(dir) = Directory::open(root, &cloud.cloud_id, false)? else {
            return Ok(None);
        };
        if !dir.exists(DATABASE)? {
            return Ok(None);
        }
        let lease = if maintenance {
            Some(Lock::acquire(&dir, WRITER, true, until, session)?)
        } else {
            None
        };
        let gate = Lock::acquire(&dir, GATE, maintenance, until, session)?;
        let db = dir.file(DATABASE, false, false)?;
        no_sidecars(&dir, DATABASE)?;
        check_header(&db)?;
        let mut conn = Connection::open_with_flags(
            dir.path.join(DATABASE),
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(sql_error)?;
        dir.check_file(DATABASE, &db)?;
        gate.check(&dir)?;
        // No implicit SQLite busy timeout may outlive the shared read deadline.
        deadline(&mut conn, until, session)?;
        configure(&conn, false)?;
        conn.execute_batch("BEGIN DEFERRED").map_err(sql_error)?;
        validate(&conn, cloud)?;
        if maintenance {
            conn.execute_batch("COMMIT").map_err(sql_error)?;
        }
        dir.check_file(DATABASE, &db)?;
        if Instant::now() >= until {
            return Err(failure("archive read deadline exceeded"));
        }
        Ok(Some(Self {
            conn,
            dir,
            db,
            _gate: gate,
            _lease: lease,
            until,
            session: session.clone(),
        }))
    }
    pub fn finish(self) -> Result<(), CliError> {
        cancelled(&self.session)?;
        if Instant::now() >= self.until {
            return Err(failure("archive read deadline exceeded"));
        }
        self.dir.check_file(DATABASE, &self.db)?;
        self._gate.check(&self.dir)?;
        if let Some(lease) = &self._lease {
            lease.check(&self.dir)?;
        }
        if !self.conn.is_autocommit() {
            self.conn.execute_batch("COMMIT").map_err(sql_error)?;
        }
        if Instant::now() >= self.until {
            return Err(failure("archive read deadline exceeded"));
        }
        Ok(())
    }
}

/// The archive's mutation generation: its incarnation (`archive_generation`) in the high
/// 32 bits and SQLite's file change counter (header offset 24) in the low 32. Every
/// committed write transaction advances the counter in the archive's rollback-journal
/// mode, whatever it changed (a new fact, a link inside an existing run, an update or a
/// prune). A file published by compaction (`VACUUM INTO` restarts the counter) or
/// produced by recovery receives a fresh random incarnation first, so a generation
/// observed before can never denote different contents later (M5 review 2 finding 1).
/// Read while the caller's read or write transaction holds its lock, it is coherent with
/// exactly the data that transaction sees. A version-1 archive (no incarnation yet, read
/// before any writer upgraded it) has incarnation 0.
fn generation_of(conn: &Connection, db: &File) -> Result<i64, CliError> {
    use std::os::unix::fs::FileExt;
    let mut counter = [0u8; 4];
    db.read_exact_at(&mut counter, 24)
        .map_err(|_| failure("cannot read the archive's mutation generation"))?;
    let incarnation: i64 = if schema_version(conn)? >= 2 {
        conn.query_row(
            "SELECT incarnation FROM archive_generation WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .map_err(|_| failure("the archive has no incarnation"))?
    } else {
        0
    };
    Ok((incarnation << 32) | i64::from(u32::from_be_bytes(counter)))
}

/// The first incarnation of a new archive lineage (a created or upgraded archive): a
/// random value in 1..2^30, so a recreated archive at the same path is unlikely to start
/// where a deleted one stood (a 2^-30 chance, not a guarantee).
fn first_incarnation() -> i64 {
    1 + i64::from((uuid::Uuid::new_v4().as_u128() as u32) & 0x3fff_ffff)
}

/// The next incarnation of an archive: strictly greater than every incarnation that
/// archive has had (a durable monotonic counter kept in the archive itself, advanced by
/// compaction and recovery), so within an archive's lineage a generation never repeats.
/// Bounded at 2^31 - 1 and refused at exhaustion rather than wrapped.
fn next_incarnation(current: i64) -> Result<i64, CliError> {
    if current == 0 {
        return Ok(first_incarnation());
    }
    let next = current + 1;
    if !(1..=0x7fff_ffff).contains(&next) {
        return Err(failure("the archive incarnation counter is exhausted"));
    }
    Ok(next)
}

/// Give a database a fresh incarnation, inside the caller's transaction (or its own).
pub(crate) fn renew_incarnation(conn: &Connection) -> Result<(), CliError> {
    let current: i64 = conn
        .query_row(
            "SELECT incarnation FROM archive_generation WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .map_err(|_| failure("the archive has no incarnation"))?;
    conn.execute(
        "UPDATE archive_generation SET incarnation=?1 WHERE singleton=1",
        [next_incarnation(current)?],
    )
    .map_err(sql_error)?;
    Ok(())
}

impl Reader {
    /// The mutation generation of the snapshot this reader's transaction sees.
    pub fn generation(&self) -> Result<i64, CliError> {
        if self.conn.is_autocommit() {
            return Err(failure(
                "the archive generation needs an open read transaction",
            ));
        }
        generation_of(&self.conn, &self.db)
    }
}

pub struct Writer {
    pub conn: Connection,
    pub dir: Directory,
    db: File,
    _gate: Lock,
    _lease: Option<Lock>,
    pub until: Instant,
    session: scm::Session,
    /// The generation a version-1 (accepted-M4) archive had when this writer upgraded
    /// it (after its verified pre-upgrade copy): the upgrade is this writer's own change,
    /// so a binding to that generation still holds ([`Writer::bound_to`]).
    upgraded_from: Option<i64>,
}

impl Writer {
    /// The mutation generation the archive holds, read inside this writer's open write
    /// transaction (`BEGIN IMMEDIATE`): no other writer can commit until this one ends,
    /// so a binding checked here holds for the whole transaction.
    pub fn generation(&self) -> Result<i64, CliError> {
        if self.conn.is_autocommit() {
            return Err(failure(
                "the archive generation needs an open write transaction",
            ));
        }
        generation_of(&self.conn, &self.db)
    }

    /// Whether the archive is still exactly what a binding to `expected` saw: its
    /// generation is unchanged, or this writer's own upgrade of the version-1 archive
    /// at that generation is the only change (inside an open write transaction).
    pub fn bound_to(&self, expected: i64) -> Result<bool, CliError> {
        Ok(self.generation()? == expected || self.upgraded_from == Some(expected))
    }
}

/// Invocation-wide writer/network lease, without a SQLite connection or batch gate.
pub struct Lease {
    dir: Directory,
    lock: Lock,
    until: Instant,
    session: scm::Session,
}
impl Lease {
    pub fn check_identity(&self) -> Result<(), CliError> {
        self.lock.check(&self.dir)
    }
    pub fn check(&self) -> Result<(), CliError> {
        cancelled(&self.session)?;
        if Instant::now() >= self.until {
            return Err(failure("archive invocation deadline exceeded"));
        }
        self.check_identity()
    }
}

impl Writer {
    /// Every mutating batch takes the exclusive gate before any SQLite open.
    /// This prevents another cooperating writer from leaving a hot journal between
    /// sidecar admission and SQLite's first read. Never hold this object over HTTP.
    pub fn open(
        root: &Path,
        cloud: &Cloud,
        budget: Duration,
        maintenance: bool,
        session: &scm::Session,
    ) -> Result<Self, CliError> {
        Self::open_inner(
            root,
            cloud,
            budget,
            maintenance,
            session,
            (true, false, true),
        )
    }
    /// An existing archive's writer that never upgrades it: a version-1 archive stays
    /// byte for byte unchanged (validation of a destination before anything may change).
    pub fn existing_unupgraded(
        root: &Path,
        cloud: &Cloud,
        budget: Duration,
        session: &scm::Session,
    ) -> Result<Self, CliError> {
        Self::open_inner(root, cloud, budget, true, session, (false, false, false))
    }
    pub fn existing(
        root: &Path,
        cloud: &Cloud,
        budget: Duration,
        session: &scm::Session,
    ) -> Result<Self, CliError> {
        Self::open_inner(root, cloud, budget, true, session, (false, false, true))
    }
    pub fn existing_batch(
        root: &Path,
        cloud: &Cloud,
        budget: Duration,
        session: &scm::Session,
    ) -> Result<Self, CliError> {
        Self::open_inner(root, cloud, budget, false, session, (false, false, true))
    }
    pub fn sync(
        root: &Path,
        cloud: &Cloud,
        budget: Duration,
        session: &scm::Session,
    ) -> Result<Self, CliError> {
        Self::open_inner(root, cloud, budget, true, session, (true, true, true))
    }
    fn open_inner(
        root: &Path,
        cloud: &Cloud,
        budget: Duration,
        maintenance: bool,
        session: &scm::Session,
        modes: (bool, bool, bool),
    ) -> Result<Self, CliError> {
        let (create, fail_fast, upgrade) = modes;
        let until = Instant::now() + budget;
        cancelled(session)?;
        heap_policy()?;
        let dir = Directory::open(root, &cloud.cloud_id, create)?
            .ok_or_else(|| failure("cannot initialize archive directory"))?;
        let present = dir.exists(DATABASE)?;
        if !present && !create {
            return Err(failure(
                "archive is missing; maintenance never initializes it",
            ));
        }
        for name in [WRITER, GATE] {
            if !dir.exists(name)? {
                if present {
                    return Err(failure(
                        "archive lock files are missing; readers and existing-archive writers never repair them",
                    ));
                }
                // Another initializer may create the same lock; admit it normally.
                if dir.file(name, true, true).is_err() && !dir.exists(name)? {
                    return Err(failure("cannot create private archive locks"));
                }
            }
        }
        let lease = if maintenance {
            Some(if fail_fast {
                Lock::try_acquire(&dir, WRITER)?
            } else {
                Lock::acquire(&dir, WRITER, true, until, session)?
            })
        } else {
            None
        };
        let gate = Lock::acquire(&dir, GATE, true, until, session)?;
        initialize_if_missing(&dir, cloud, until, session, create, present)?;
        let db = dir.file(DATABASE, true, false)?;
        no_sidecars(&dir, DATABASE)?;
        check_header(&db)?;
        let mut conn = Connection::open_with_flags(
            dir.path.join(DATABASE),
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(sql_error)?;
        dir.check_file(DATABASE, &db)?;
        gate.check(&dir)?;
        if let Some(lease) = &lease {
            lease.check(&dir)?;
        }
        deadline(&mut conn, until, session)?;
        configure(&conn, true)?;
        validate(&conn, cloud)?;
        // An accepted-M4 (version 1) archive is upgraded in place, under this writer's
        // locks, before anything else writes to it, and only after a verified private
        // copy of the version-1 file and its manifest are durable (the upgrade's backup).
        let upgraded_from = if upgrade && schema_version(&conn)? == 1 {
            let before = generation_of(&conn, &db)?;
            backup_before_upgrade(&dir, &db, cloud, until, session)?;
            upgrade_v1(&conn, cloud)?;
            Some(before)
        } else {
            None
        };
        dir.check_file(DATABASE, &db)?;
        if Instant::now() >= until {
            return Err(failure("archive writer deadline exceeded"));
        }
        Ok(Self {
            conn,
            dir,
            db,
            _gate: gate,
            _lease: lease,
            until,
            session: session.clone(),
            upgraded_from,
        })
    }
    pub fn check(&self) -> Result<(), CliError> {
        cancelled(&self.session)?;
        if Instant::now() >= self.until {
            return Err(failure("archive write deadline exceeded"));
        }
        self._gate.check(&self.dir)?;
        if let Some(lease) = &self._lease {
            lease.check(&self.dir)?;
        }
        self.dir.check_file(DATABASE, &self.db)
    }
    pub fn into_lease(self) -> Result<Lease, CliError> {
        self.check()?;
        let Self {
            conn,
            dir,
            db,
            _gate,
            _lease,
            until,
            session,
            ..
        } = self;
        drop(conn);
        drop(db);
        drop(_gate);
        let lock = _lease.ok_or_else(|| failure("an invocation lease was not acquired"))?;
        Ok(Lease {
            dir,
            lock,
            until,
            session,
        })
    }
}

fn initialize_if_missing(
    dir: &Directory,
    cloud: &Cloud,
    until: Instant,
    session: &scm::Session,
    create: bool,
    previously_present: bool,
) -> Result<(), CliError> {
    if !dir.exists(DATABASE)? {
        if previously_present || !create {
            return Err(failure(
                "archive disappeared while waiting; it was not recreated",
            ));
        }
        initialize(dir, cloud, until, session)?;
    }
    Ok(())
}

fn initialize(
    dir: &Directory,
    cloud: &Cloud,
    until: Instant,
    session: &scm::Session,
) -> Result<(), CliError> {
    let name = format!(
        ".archive.{}.initializing.sqlite3",
        uuid::Uuid::new_v4().simple()
    );
    let file = dir.file(&name, true, true)?;
    let result = (|| {
        let mut conn = Connection::open_in_memory().map_err(sql_error)?;
        deadline(&mut conn, until, session)?;
        configure(&conn, true)?;
        conn.execute_batch("BEGIN IMMEDIATE").map_err(sql_error)?;
        conn.execute_batch(SCHEMA).map_err(sql_error)?;
        conn.execute("INSERT INTO archive_meta(singleton,cloud_id,tsg_id,environment_id,environment_json,created_ms,analysis_version,redaction_version) VALUES (1,?1,?2,?3,?4,?5,1,1)",params![cloud.cloud_id,cloud.tsg,cloud.environment_id,cloud.descriptor()?,now_ms()?]).map_err(sql_error)?;
        conn.execute(
            "INSERT INTO archive_generation VALUES (1, ?1)",
            [first_incarnation()],
        )
        .map_err(sql_error)?;
        conn.execute_batch("COMMIT").map_err(sql_error)?;
        validate_content(&conn, cloud)?;
        let image = conn.serialize(rusqlite::MAIN_DB).map_err(sql_error)?;
        dir.check_file(&name, &file)?;
        use std::io::Write;
        let mut destination = &file;
        destination
            .write_all(&image)
            .map_err(|_| failure("cannot write initialized archive"))?;
        check_header(&file)?;
        dir.check_file(&name, &file)?;
        if Instant::now() >= until || session.is_cancelled() {
            return Err(failure(
                "archive initialization deadline exceeded or interrupted",
            ));
        }
        file.sync_all()
            .map_err(|_| failure("archive initialization durability could not be confirmed"))?;
        dir.cloud
            .sync_all()
            .map_err(|_| failure("archive staging-directory durability could not be confirmed"))?;
        dir.recheck()?;
        match rustix::fs::renameat_with(
            &dir.cloud,
            &name,
            &dir.cloud,
            DATABASE,
            rustix::fs::RenameFlags::NOREPLACE,
        ) {
            Ok(()) => dir.cloud.sync_all().map_err(|_| {
                failure("archive was published but directory durability is uncertain")
            })?,
            Err(rustix::io::Errno::EXIST) => {} // Other successful initializer; validate it on open.
            Err(_) => return Err(failure("cannot publish initialized archive")),
        }
        Ok(())
    })();
    // Delete only the inode this operation created. Failed SQLite journals remain
    // private recovery material; never unlink a replacement or another operation's file.
    if dir.check_file(&name, &file).is_ok() {
        let journal = format!("{name}-journal");
        if !dir.exists(&journal).unwrap_or(true) {
            let _ = rustix::fs::unlinkat(&dir.cloud, &name, rustix::fs::AtFlags::empty());
        }
    }
    result
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MaintenanceStep {
    BeforeCopy,
    AfterCopy,
    BeforePublish,
    AfterPublish,
    DirectorySync,
}
#[cfg(test)]
thread_local! {static FAIL_MAINTENANCE:std::cell::Cell<Option<MaintenanceStep>>=const{std::cell::Cell::new(None)};}
fn maintenance_step(step: MaintenanceStep) -> Result<(), CliError> {
    #[cfg(test)]
    if FAIL_MAINTENANCE.with(|v| v.get() == Some(step)) {
        return Err(failure("injected maintenance failure"));
    }
    let _ = step;
    Ok(())
}

fn space(dir: &Directory, required: u128) -> Result<(), CliError> {
    let stat = rustix::fs::fstatvfs(&dir.cloud)
        .map_err(|_| failure("cannot determine archive free space"))?;
    let free = u128::from(stat.f_bavail) * u128::from(stat.f_frsize);
    if free < required {
        return Err(failure(
            "insufficient free space for verified archive maintenance",
        ));
    }
    Ok(())
}

fn integrity(conn: &Connection) -> Result<(), CliError> {
    let result: String = conn
        .query_row("PRAGMA integrity_check(1)", [], |r| r.get(0))
        .map_err(sql_error)?;
    if result == "ok"
        && !conn
            .prepare("PRAGMA foreign_key_check")
            .map_err(sql_error)?
            .exists([])
            .map_err(sql_error)?
    {
        Ok(())
    } else {
        Err(failure(
            "archive integrity check failed; preserve existing data and recovery artifacts",
        ))
    }
}

fn cleanup_owned(dir: &Directory, name: &str, file: &File) {
    if dir.check_file(name, file).is_ok() {
        let _ = rustix::fs::unlinkat(&dir.cloud, name, rustix::fs::AtFlags::empty());
    }
}

pub fn compact(
    root: &Path,
    cloud: &Cloud,
    budget: Duration,
    session: &scm::Session,
) -> Result<serde_json::Value, CliError> {
    let w = Writer::existing(root, cloud, budget, session)?;
    let before =
        w.db.metadata()
            .map_err(|_| failure("cannot inspect archive size"))?
            .len();
    space(&w.dir, u128::from(before) * 2)?;
    integrity(&w.conn)?;
    let name = format!(".compact.{}.sqlite3", uuid::Uuid::new_v4().simple());
    let copy = w.dir.file(&name, true, true)?;
    let mut published = false;
    let result = (|| {
        w.check()?;
        maintenance_step(MaintenanceStep::BeforeCopy)?;
        w.dir.check_file(&name, &copy)?;
        w.conn
            .set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_ATTACHED, 1)
            .map_err(sql_error)?;
        let path = w.dir.path.join(&name);
        w.conn
            .execute(
                "VACUUM INTO ?1",
                [path
                    .to_str()
                    .ok_or_else(|| failure("compaction path must be valid UTF-8"))?],
            )
            .map_err(sql_error)?;
        w.conn
            .set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_ATTACHED, 0)
            .map_err(sql_error)?;
        maintenance_step(MaintenanceStep::AfterCopy)?;
        w.check()?;
        w.dir.check_file(&name, &copy)?;
        no_sidecars(&w.dir, &name)?;
        check_header(&copy)?;
        // `VACUUM INTO` restarts the change counter and copies the incarnation: the copy
        // gets a fresh incarnation before it can be published.
        {
            let mut fresh = Connection::open_with_flags(
                &path,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX
                    | OpenFlags::SQLITE_OPEN_NOFOLLOW,
            )
            .map_err(sql_error)?;
            w.dir.check_file(&name, &copy)?;
            deadline(&mut fresh, w.until, session)?;
            configure(&fresh, true)?;
            renew_incarnation(&fresh)?;
        }
        w.dir.check_file(&name, &copy)?;
        no_sidecars(&w.dir, &name)?;
        check_header(&copy)?;
        let mut verify = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(sql_error)?;
        w.dir.check_file(&name, &copy)?;
        deadline(&mut verify, w.until, session)?;
        configure(&verify, false)?;
        validate(&verify, cloud)?;
        integrity(&verify)?;
        drop(verify);
        copy.sync_all()
            .map_err(|_| failure("compacted copy durability could not be confirmed"))?;
        w.dir.cloud.sync_all().map_err(|_| {
            failure("compaction staging directory durability could not be confirmed")
        })?;
        maintenance_step(MaintenanceStep::BeforePublish)?;
        w.check()?;
        w.dir.check_file(&name, &copy)?;
        // Close the SQLite writer before its inode is replaced, retaining both locks.
        let Writer {
            conn,
            dir,
            db,
            _gate,
            _lease,
            until,
            session: _,
            upgraded_from: _,
        } = w;
        drop(conn);
        dir.check_file(DATABASE, &db)?;
        dir.check_file(&name, &copy)?;
        if Instant::now() >= until || session.is_cancelled() {
            return Err(failure("compaction interrupted before publication"));
        }
        rustix::fs::renameat(&dir.cloud, &name, &dir.cloud, DATABASE)
            .map_err(|_| failure("cannot publish verified compacted archive"))?;
        published = true;
        if maintenance_step(MaintenanceStep::AfterPublish).is_err()
            || maintenance_step(MaintenanceStep::DirectorySync).is_err()
            || dir.cloud.sync_all().is_err()
        {
            return Err(failure(
                "compacted archive was published but directory durability is uncertain; do not overwrite it with an older copy",
            ));
        }
        dir.check_file(DATABASE, &copy)?;
        let after = copy
            .metadata()
            .map_err(|_| failure("cannot inspect compacted size"))?
            .len();
        drop(_gate);
        drop(_lease);
        Ok(
            serde_json::json!({"kind":"runtime_history_compact","before_bytes":before,"after_bytes":after,"durable":true}),
        )
    })();
    // The original writer is consumed only at publication. Re-open existing
    // descriptors for owned cleanup; never initialize or unlink replacements.
    if !published && let Ok(Some(dir)) = Directory::open(root, &cloud.cloud_id, false) {
        cleanup_owned(&dir, &name, &copy);
        let _ = dir.cloud.sync_all();
    }
    result
}

fn file_record(
    name: &str,
    file: &File,
    until: Instant,
    session: &scm::Session,
) -> Result<serde_json::Value, CliError> {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::{FileExt, MetadataExt};
    let before = file
        .metadata()
        .map_err(|_| failure("cannot inspect maintenance source"))?;
    let mut hash = Sha256::new();
    let mut offset = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    while offset < before.len() {
        cancelled(session)?;
        if Instant::now() >= until {
            return Err(failure("maintenance checksum deadline exceeded"));
        }
        let length = usize::try_from((before.len() - offset).min(buffer.len() as u64))
            .map_err(|_| failure("maintenance source is too large"))?;
        let n = file
            .read_at(&mut buffer[..length], offset)
            .map_err(|_| failure("cannot checksum maintenance source"))?;
        if n == 0 {
            return Err(failure("maintenance source changed during checksum"));
        }
        hash.update(&buffer[..n]);
        offset += n as u64;
    }
    let after = file
        .metadata()
        .map_err(|_| failure("cannot recheck maintenance source"))?;
    if (
        before.len(),
        before.mtime(),
        before.mtime_nsec(),
        before.ctime(),
        before.ctime_nsec(),
    ) != (
        after.len(),
        after.mtime(),
        after.mtime_nsec(),
        after.ctime(),
        after.ctime_nsec(),
    ) {
        return Err(failure("maintenance source changed during checksum"));
    }
    Ok(
        serde_json::json!({"file":name,"bytes":before.len(),"sha256":format!("{:x}",hash.finalize())}),
    )
}
fn same_content(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    a["bytes"] == b["bytes"] && a["sha256"] == b["sha256"]
}
fn copy_backup(
    dir: &Directory,
    name: &str,
    source: &File,
    until: Instant,
    session: &scm::Session,
) -> Result<serde_json::Value, CliError> {
    use sha2::{Digest, Sha256};
    use std::{
        io::Write,
        os::unix::fs::{FileExt, MetadataExt},
    };
    let target = dir.file(name, true, true)?;
    let before = source
        .metadata()
        .map_err(|_| failure("cannot inspect recovery source"))?;
    let expected = before.len();
    let mut hash = Sha256::new();
    let mut offset = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    let mut destination = &target;
    while offset < expected {
        cancelled(session)?;
        if Instant::now() >= until {
            return Err(failure(
                "recovery backup deadline exceeded; source was not recovered",
            ));
        }
        let length = usize::try_from((expected - offset).min(buffer.len() as u64))
            .map_err(|_| failure("recovery source size is unsupported"))?;
        let n = source
            .read_at(&mut buffer[..length], offset)
            .map_err(|_| failure("cannot copy recovery source"))?;
        if n == 0 {
            return Err(failure("recovery source changed during backup"));
        }
        destination
            .write_all(&buffer[..n])
            .map_err(|_| failure("cannot write private recovery backup"))?;
        hash.update(&buffer[..n]);
        offset += n as u64;
    }
    let after = source
        .metadata()
        .map_err(|_| failure("cannot recheck recovery source"))?;
    if (
        before.len(),
        before.mtime(),
        before.mtime_nsec(),
        before.ctime(),
        before.ctime_nsec(),
    ) != (
        after.len(),
        after.mtime(),
        after.mtime_nsec(),
        after.ctime(),
        after.ctime_nsec(),
    ) {
        return Err(failure("recovery source changed during backup"));
    }
    dir.check_file(name, &target)?;
    target
        .sync_all()
        .map_err(|_| failure("recovery backup durability could not be confirmed"))?;
    let expected =
        serde_json::json!({"file":name,"bytes":expected,"sha256":format!("{:x}",hash.finalize())});
    let verified = file_record(name, &target, until, session)?;
    if !same_content(&expected, &verified) {
        return Err(failure(
            "private recovery backup checksum differs; live archive was not recovered",
        ));
    }
    Ok(verified)
}
fn admit_journal(journal: &File, db_len: u64) -> Result<(), CliError> {
    use std::os::unix::fs::FileExt;
    let length = journal
        .metadata()
        .map_err(|_| failure("cannot inspect rollback journal"))?
        .len();
    if length > db_len.saturating_mul(2).saturating_add(1024 * 1024) {
        return Err(failure(
            "rollback journal exceeds the supported recovery bound; files were preserved",
        ));
    }
    let magic = [0xd9, 0xd5, 0x05, 0xf9, 0x20, 0xa1, 0x63, 0xd7];
    if length >= 8 {
        let mut tail = [0; 8];
        journal
            .read_exact_at(&mut tail, length - 8)
            .map_err(|_| failure("cannot inspect journal trailer"))?;
        if tail == magic {
            return Err(failure(
                "super-journal references are unsupported; archive and journal were left unchanged",
            ));
        }
        let mut header = [0; 8];
        journal
            .read_exact_at(&mut header, 0)
            .map_err(|_| failure("cannot inspect journal header"))?;
        if header != magic && header != [0; 8] {
            return Err(failure(
                "rollback journal header is unsupported; files were preserved",
            ));
        }
    } else if length != 0 {
        return Err(failure(
            "truncated rollback journal was preserved for inspection",
        ));
    }
    Ok(())
}
fn recover_file(
    dir: &Directory,
    name: &str,
    file: &File,
    journal: Option<&File>,
    cloud: &Cloud,
    until: Instant,
    session: &scm::Session,
) -> Result<(), CliError> {
    use std::os::unix::fs::FileExt;
    let journal_name = format!("{name}-journal");
    if let Some(journal) = journal {
        dir.check_file(&journal_name, journal)?;
        admit_journal(
            journal,
            file.metadata()
                .map_err(|_| failure("cannot inspect recovery database"))?
                .len(),
        )?;
    } else if dir.exists(&journal_name)? {
        return Err(failure("unexpected journal appeared before recovery"));
    }
    for suffix in ["-wal", "-shm"] {
        if dir.exists(&format!("{name}{suffix}"))? {
            return Err(failure("WAL/SHM recovery is unsupported"));
        }
    }
    cancelled(session)?;
    if Instant::now() >= until {
        return Err(failure("recovery deadline exceeded before SQLite access"));
    }
    dir.check_file(name, file)?;
    check_header(file)?;
    let mut conn = Connection::open_with_flags(
        dir.path.join(name),
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(sql_error)?;
    dir.check_file(name, file)?;
    deadline(&mut conn, until, session)?;
    configure(&conn, true)?;
    conn.execute_batch("BEGIN EXCLUSIVE").map_err(sql_error)?;
    validate(&conn, cloud)?;
    integrity(&conn)?;
    if dir.exists(&journal_name)? {
        let old = journal.ok_or_else(|| failure("unexpected journal appeared during recovery"))?;
        dir.check_file(&journal_name, old)?;
        let length = old
            .metadata()
            .map_err(|_| failure("cannot inspect remaining journal"))?
            .len();
        let mut header = [0; 8];
        if length != 0 {
            old.read_exact_at(&mut header, 0)
                .map_err(|_| failure("remaining journal is truncated"))?;
        }
        if header != [0; 8] {
            return Err(failure(
                "journal remains after recovery; preserve archive and private backups",
            ));
        }
        rustix::fs::unlinkat(&dir.cloud, &journal_name, rustix::fs::AtFlags::empty())
            .map_err(|_| failure("cannot remove backed-up non-hot journal"))?;
    }
    cancelled(session)?;
    if Instant::now() >= until {
        return Err(failure("recovery deadline exceeded after validation"));
    }
    // A file a journal was rolled back into is a new incarnation: no generation observed
    // before recovery can denote its contents. A healthy archive (no journal) is left
    // byte for byte unchanged.
    if journal.is_some() {
        if schema_version(&conn)? == 1 {
            // A recovered accepted-M4 archive becomes version 2 in the same verified
            // transaction (its first incarnation); the raw pre-recovery copies kept by
            // the caller are the upgrade's backup.
            upgrade_v1_in_transaction(&conn, cloud)?;
        } else {
            renew_incarnation(&conn)?;
        }
    }
    conn.execute_batch("COMMIT").map_err(sql_error)?;
    drop(conn);
    file.sync_all()
        .map_err(|_| failure("recovered database durability could not be confirmed"))?;
    dir.cloud
        .sync_all()
        .map_err(|_| failure("recovery directory durability could not be confirmed"))?;
    dir.check_file(name, file)?;
    no_sidecars(dir, name)
}
fn manifest(dir: &Directory, name: &str, value: &serde_json::Value) -> Result<(), CliError> {
    use std::io::Write;
    let file = dir.file(name, true, true)?;
    let mut destination = &file;
    destination
        .write_all(
            serde_json::to_string_pretty(value)
                .map_err(|_| failure("cannot encode private recovery manifest"))?
                .as_bytes(),
        )
        .map_err(|_| failure("cannot write private recovery manifest"))?;
    dir.check_file(name, &file)?;
    file.sync_all()
        .map_err(|_| failure("private recovery manifest durability could not be confirmed"))?;
    dir.cloud.sync_all().map_err(|_| {
        failure("private recovery manifest directory durability could not be confirmed")
    })
}

/// The identity a guarded recovery is bound to (M5 review 2 finding 9): the archive's
/// and its rollback journal's file identities (device, inode, length, modification time)
/// and headers, read without admitting either to SQLite (the ordinary reader refuses a
/// journal sidecar). Any change to either file between a recovery preview and its
/// execution changes it. The journal must be admissible (bounded, regular, owned) and
/// WAL/SHM sidecars refuse, exactly as [`recover`] checks before it runs.
pub fn recovery_identity(
    root: &Path,
    cloud: &Cloud,
    budget: Duration,
    session: &scm::Session,
) -> Result<i64, CliError> {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::{FileExt, MetadataExt};
    cancelled(session)?;
    let until = Instant::now() + budget;
    let dir = Directory::open(root, &cloud.cloud_id, false)?
        .ok_or_else(|| failure("archive is missing; recovery never initializes it"))?;
    let gate = Lock::acquire(&dir, GATE, false, until, session)?;
    for name in ["archive.sqlite3-wal", "archive.sqlite3-shm"] {
        if dir.exists(name)? {
            return Err(failure(
                "WAL/SHM recovery is unsupported; files were left unchanged",
            ));
        }
    }
    let db = dir.file(DATABASE, false, false)?;
    check_header(&db)?;
    let mut digest = Sha256::new();
    let add = |digest: &mut Sha256, file: &File, header: usize| -> Result<(), CliError> {
        let m = file
            .metadata()
            .map_err(|_| failure("cannot inspect the archive for recovery"))?;
        digest.update(m.dev().to_be_bytes());
        digest.update(m.ino().to_be_bytes());
        digest.update(m.len().to_be_bytes());
        digest.update(m.mtime().to_be_bytes());
        digest.update(m.mtime_nsec().to_be_bytes());
        let mut bytes = vec![0u8; header];
        let n = file
            .read_at(&mut bytes, 0)
            .map_err(|_| failure("cannot read the archive for recovery"))?;
        digest.update(&bytes[..n]);
        Ok(())
    };
    add(&mut digest, &db, 100)?;
    let journal_name = "archive.sqlite3-journal";
    if dir.exists(journal_name)? {
        let journal = dir.file(journal_name, false, false)?;
        let db_len = db
            .metadata()
            .map_err(|_| failure("cannot inspect recovery database"))?
            .len();
        admit_journal(&journal, db_len)?;
        digest.update(b"journal");
        add(&mut digest, &journal, 512)?;
    }
    dir.check_file(DATABASE, &db)?;
    gate.check(&dir)?;
    let bytes = digest.finalize();
    Ok(i64::from_be_bytes(bytes[..8].try_into().expect("eight bytes")) & i64::MAX)
}

pub fn recover(
    root: &Path,
    cloud: &Cloud,
    budget: Duration,
    session: &scm::Session,
) -> Result<serde_json::Value, CliError> {
    cancelled(session)?;
    let until = Instant::now() + budget;
    heap_policy()?;
    let dir = Directory::open(root, &cloud.cloud_id, false)?
        .ok_or_else(|| failure("archive is missing; recovery never initializes it"))?;
    let lease = Lock::acquire(&dir, WRITER, true, until, session)?;
    let gate = Lock::acquire(&dir, GATE, true, until, session)?;
    let db = dir.file(DATABASE, true, false)?;
    check_header(&db)?;
    for name in ["archive.sqlite3-wal", "archive.sqlite3-shm"] {
        if dir.exists(name)? {
            let _ = dir.file(name, false, false)?;
            return Err(failure(
                "WAL/SHM recovery is unsupported; files were left unchanged",
            ));
        }
    }
    let journal_name = "archive.sqlite3-journal";
    let journal = if dir.exists(journal_name)? {
        Some(dir.file(journal_name, false, false)?)
    } else {
        None
    };
    let mut backups = serde_json::Value::Null;
    if let Some(journal) = &journal {
        let db_len = db
            .metadata()
            .map_err(|_| failure("cannot inspect recovery database"))?
            .len();
        let j_len = journal
            .metadata()
            .map_err(|_| failure("cannot inspect rollback journal"))?
            .len();
        admit_journal(journal, db_len)?;
        space(
            &dir,
            2 * (u128::from(db_len) + u128::from(j_len)) + 1024 * 1024,
        )?;
        let id = uuid::Uuid::new_v4().simple().to_string();
        let db_name = format!(".pre-recovery.{id}.sqlite3");
        let journal_backup_name = format!(".pre-recovery.{id}.journal");
        let db_backup = copy_backup(&dir, &db_name, &db, until, session)?;
        let journal_backup = copy_backup(&dir, &journal_backup_name, journal, until, session)?;
        let original_manifest = format!(".pre-recovery.{id}.json");
        manifest(
            &dir,
            &original_manifest,
            &serde_json::json!({"format":"airs-runtime-history-recovery-backup","version":1,"schema_header_version":VERSION,"expected_cloud_id":cloud.cloud_id,"cloud_identity_verified":false,"sqlite_version":rusqlite::version(),"created_ms":now_ms()?,"database":db_backup,"journal":journal_backup}),
        )?;
        // Never recover the raw backups. A second working pair proves exact
        // cloud/schema/integrity before the live file can undergo native recovery.
        let recovered_name = format!(".pre-recovery.{id}.recovered.sqlite3");
        let recovered_journal_name = format!("{recovered_name}-journal");
        let raw_db = dir.file(&db_name, false, false)?;
        let raw_journal = dir.file(&journal_backup_name, false, false)?;
        let _ = copy_backup(&dir, &recovered_name, &raw_db, until, session)?;
        let _ = copy_backup(&dir, &recovered_journal_name, &raw_journal, until, session)?;
        let recovered = dir.file(&recovered_name, true, false)?;
        let copied_journal = dir.file(&recovered_journal_name, false, false)?;
        admit_journal(
            &copied_journal,
            recovered
                .metadata()
                .map_err(|_| failure("cannot inspect copied recovery database"))?
                .len(),
        )?;
        recover_file(
            &dir,
            &recovered_name,
            &recovered,
            Some(&copied_journal),
            cloud,
            until,
            session,
        )?;
        let verified_copy = file_record(&recovered_name, &recovered, until, session)?;
        let verified_manifest = format!(".pre-recovery.{id}.verified.json");
        manifest(
            &dir,
            &verified_manifest,
            &serde_json::json!({"format":"airs-runtime-history-verified-recovery","version":1,"schema_version":VERSION,"cloud_id":cloud.cloud_id,"sqlite_version":rusqlite::version(),"created_ms":now_ms()?,"original_manifest":original_manifest,"recovered_copy":verified_copy}),
        )?;
        // Recheck bytes, inodes and both cooperative locks immediately before the
        // first live SQLite read can trigger native rollback. No cross-cloud write.
        dir.check_file(DATABASE, &db)?;
        dir.check_file(journal_name, journal)?;
        gate.check(&dir)?;
        lease.check(&dir)?;
        if !same_content(&file_record(DATABASE, &db, until, session)?, &db_backup)
            || !same_content(
                &file_record(journal_name, journal, until, session)?,
                &journal_backup,
            )
        {
            return Err(failure(
                "live recovery sources changed after copy validation; raw backups were preserved",
            ));
        }
        admit_journal(journal, db_len)?;
        backups = serde_json::json!({"manifest":verified_manifest,"original_manifest":original_manifest,"database":db_backup,"journal":journal_backup,"verified_recovered_copy":verified_copy});
    }
    dir.check_file(DATABASE, &db)?;
    gate.check(&dir)?;
    lease.check(&dir)?;
    recover_file(&dir, DATABASE, &db, journal.as_ref(), cloud, until, session)?;
    Ok(
        serde_json::json!({"kind":"runtime_history_recover","recovery_attempted":journal.is_some(),"integrity_verified":true,"private_backups":backups}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestDir;
    use airs_runtime_core::tenant::{Selected, TenantContext, config::TenantConfig};
    use std::{
        fs,
        os::unix::fs::{FileExt, MetadataExt},
    };
    fn cloud() -> Cloud {
        Cloud::from_tenant(&TenantContext {
            selected: Selected {
                name: "fixture".into(),
                tsg_id: "123".into(),
                by_flag: false,
            },
            config: TenantConfig::default(),
        })
        .unwrap()
    }
    fn state(path: &Path) -> Vec<(String, Vec<u8>, u32, i64, i64)> {
        let mut rows = fs::read_dir(path)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                let m = e.metadata().unwrap();
                (
                    e.file_name().to_string_lossy().into_owned(),
                    fs::read(e.path()).unwrap(),
                    m.mode(),
                    m.mtime(),
                    m.mtime_nsec(),
                )
            })
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        rows
    }
    fn initialized(tag: &str) -> (TestDir, Cloud) {
        let d = TestDir::new(tag);
        let c = cloud();
        let w = Writer::open(
            d.path(),
            &c,
            Duration::from_secs(10),
            true,
            &scm::Session::new(),
        )
        .unwrap();
        w.check().unwrap();
        drop(w);
        (d, c)
    }
    #[test]
    fn initialization_and_coherent_reads_are_private_and_nonwriting() {
        let _isolation = crate::test_isolation();
        let d = TestDir::new("history-db-missing");
        let c = cloud();
        assert!(
            Reader::open(d.path(), &c, &scm::Session::new())
                .unwrap()
                .is_none()
        );
        assert!(fs::read_dir(d.path()).unwrap().next().is_none());
        let w = Writer::open(
            d.path(),
            &c,
            Duration::from_secs(10),
            true,
            &scm::Session::new(),
        )
        .unwrap();
        assert_eq!(w.db.metadata().unwrap().mode() & 0o777, 0o600);
        let location = w.dir.path.clone();
        drop(w);
        let before = state(&location);
        let read = Reader::open(d.path(), &c, &scm::Session::new())
            .unwrap()
            .unwrap();
        let n: i64 = read
            .conn
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
        read.finish().unwrap();
        assert_eq!(state(&location), before);
    }
    #[test]
    fn unsupported_corrupt_and_journal_archives_are_refused_without_writes() {
        let _isolation = crate::test_isolation();
        for kind in [
            "older",
            "newer",
            "foreign-application",
            "corrupt",
            "wal",
            "journal",
            "missing-gate",
        ] {
            let (d, c) = initialized("history-db-refuse");
            let path = d.path().join(&c.cloud_id);
            let db = path.join(DATABASE);
            match kind {
                "older" => {
                    let f = fs::OpenOptions::new().write(true).open(&db).unwrap();
                    f.write_all_at(&0u32.to_be_bytes(), 60).unwrap();
                }
                "newer" => {
                    let f = fs::OpenOptions::new().write(true).open(&db).unwrap();
                    f.write_all_at(&((VERSION + 1) as u32).to_be_bytes(), 60)
                        .unwrap();
                }
                "foreign-application" => {
                    let f = fs::OpenOptions::new().write(true).open(&db).unwrap();
                    f.write_all_at(&0u32.to_be_bytes(), 68).unwrap();
                }
                "corrupt" => {
                    let f = fs::OpenOptions::new().write(true).open(&db).unwrap();
                    f.write_all_at(b"not a database", 0).unwrap();
                }
                "wal" => {
                    let f = fs::OpenOptions::new().write(true).open(&db).unwrap();
                    f.write_all_at(&[2, 2], 18).unwrap();
                }
                "journal" => {
                    use std::os::unix::fs::OpenOptionsExt;
                    fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .open(path.join("archive.sqlite3-journal"))
                        .unwrap();
                }
                _ => fs::remove_file(path.join(GATE)).unwrap(),
            }
            let before = state(&path);
            assert!(
                Reader::open(d.path(), &c, &scm::Session::new()).is_err(),
                "{kind}"
            );
            assert!(
                Writer::open(
                    d.path(),
                    &c,
                    Duration::from_secs(1),
                    true,
                    &scm::Session::new()
                )
                .is_err(),
                "{kind}"
            );
            assert_eq!(state(&path), before, "{kind}");
        }
    }
    #[test]
    fn long_reader_bounds_retention_and_deadline_cancels_sql() {
        let _isolation = crate::test_isolation();
        let (d, c) = initialized("history-db-busy");
        let r = Reader::open(d.path(), &c, &scm::Session::new())
            .unwrap()
            .unwrap();
        let start = Instant::now();
        assert!(Writer::open(d.path(), &c, RETAIN_BUDGET, false, &scm::Session::new()).is_err());
        assert!(start.elapsed() < Duration::from_millis(350));
        drop(r);
        let session = scm::Session::new();
        let mut r = Reader::open(d.path(), &c, &session).unwrap().unwrap();
        deadline(
            &mut r.conn,
            Instant::now() + Duration::from_millis(50),
            &session,
        )
        .unwrap();
        let start = Instant::now();
        let result=r.conn.query_row("WITH RECURSIVE s(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM s WHERE n<1000000000) SELECT sum(n) FROM s",[],|r|r.get::<_,i64>(0));
        assert!(
            matches!(result,Err(rusqlite::Error::SqliteFailure(e,_)) if e.code==rusqlite::ErrorCode::OperationInterrupted)
        );
        assert!(start.elapsed() < Duration::from_millis(150));
        drop(r);
        let session = scm::Session::new();
        let r = Reader::open(d.path(), &c, &session).unwrap().unwrap();
        session.cancel();
        let result=r.conn.query_row("WITH RECURSIVE s(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM s WHERE n<1000000000) SELECT sum(n) FROM s",[],|r|r.get::<_,i64>(0));
        assert!(result.is_err());
    }
    #[test]
    fn disk_full_rolls_back_and_secure_delete_removes_live_canary() {
        let _isolation = crate::test_isolation();
        let (d, c) = initialized("history-db-rollback");
        let w = Writer::open(
            d.path(),
            &c,
            Duration::from_secs(10),
            true,
            &scm::Session::new(),
        )
        .unwrap();
        w.conn.execute("INSERT INTO events VALUES ('e','app','App','s','scan',0,NULL,1,1,1,NULL,'unknown','unknown',0)",[]).unwrap();
        let marker = "SYNTHETIC_B1_LIVE_DELETE_99770b75".repeat(100);
        let json = serde_json::json!({"note":marker}).to_string();
        w.conn
            .execute(
                "INSERT INTO event_facts VALUES ('fact','e','session_detail',1,1,?1)",
                [&json],
            )
            .unwrap();
        let db = w.dir.path.join(DATABASE);
        assert!(
            fs::read(&db)
                .unwrap()
                .windows(30)
                .any(|s| s == &marker.as_bytes()[..30])
        );
        w.conn.execute("DELETE FROM events", []).unwrap();
        assert!(
            !fs::read(&db)
                .unwrap()
                .windows(30)
                .any(|s| s == &marker.as_bytes()[..30])
        );
        assert!(!w.dir.exists("archive.sqlite3-journal").unwrap());
        let pages: i64 = w
            .conn
            .query_row("PRAGMA page_count", [], |r| r.get(0))
            .unwrap();
        w.conn.pragma_update(None, "max_page_count", pages).unwrap();
        w.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        w.conn.execute("INSERT INTO events VALUES ('new','app','App','s','scan',0,NULL,1,1,1,NULL,'unknown','unknown',0)",[]).unwrap();
        let huge = serde_json::json!({"note":"x".repeat(512*1024)}).to_string();
        let failed = w.conn.execute(
            "INSERT INTO event_facts VALUES ('full','new','session_detail',1,1,?1)",
            [huge],
        );
        assert!(
            matches!(failed,Err(rusqlite::Error::SqliteFailure(e,_)) if e.code==rusqlite::ErrorCode::DiskFull)
        );
        if !w.conn.is_autocommit() {
            w.conn.execute_batch("ROLLBACK").unwrap();
        }
        let count: i64 = w
            .conn
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        drop(w);
        Reader::open(d.path(), &c, &scm::Session::new())
            .unwrap()
            .unwrap()
            .finish()
            .unwrap();
    }

    #[test]
    fn fifo_probe_child() {
        let _isolation = crate::test_isolation();
        let Some(path) = std::env::var_os("AIRS_HISTORY_FIFO_TEST_ROOT") else {
            return;
        };
        let path = std::path::PathBuf::from(path);
        assert!(path.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("airs-history-fifo-")
        );
        assert!(Reader::open(&path, &cloud(), &scm::Session::new()).is_err());
    }

    #[test]
    fn fifos_at_gate_database_and_journal_never_block_admission() {
        let _isolation = crate::test_isolation();
        for name in [GATE, DATABASE, "archive.sqlite3-journal"] {
            let (d, c) = initialized("history-fifo-parent");
            let dir = Directory::open(d.path(), &c.cloud_id, false)
                .unwrap()
                .unwrap();
            if dir.exists(name).unwrap() {
                fs::remove_file(dir.path.join(name)).unwrap();
            }
            assert!(
                std::process::Command::new("mkfifo")
                    .args(["-m", "600"])
                    .arg(dir.path.join(name))
                    .status()
                    .unwrap()
                    .success()
            );
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "database::tests::fifo_probe_child",
                    "--nocapture",
                ])
                .env("AIRS_HISTORY_FIFO_TEST_ROOT", d.path())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let until = Instant::now() + Duration::from_secs(2);
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(status.success(), "FIFO child failed for {name}");
                    break;
                }
                if !pause(until) {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("FIFO blocked archive admission at {name}");
                }
            }
        }
    }

    #[test]
    fn a_database_removed_after_admission_is_never_reinitialized() {
        let _isolation = crate::test_isolation();
        for create in [true, false] {
            let (d, c) = initialized("history-disappeared");
            let dir = Directory::open(d.path(), &c.cloud_id, false)
                .unwrap()
                .unwrap();
            let session = scm::Session::new();
            let _gate = Lock::acquire(
                &dir,
                GATE,
                true,
                Instant::now() + Duration::from_secs(1),
                &session,
            )
            .unwrap();
            fs::remove_file(dir.path.join(DATABASE)).unwrap();
            let before = state(&dir.path);
            assert!(
                initialize_if_missing(
                    &dir,
                    &c,
                    Instant::now() + Duration::from_secs(1),
                    &session,
                    create,
                    true
                )
                .is_err()
            );
            assert_eq!(state(&dir.path), before);
            assert!(!dir.path.join(DATABASE).exists());
        }
    }

    #[test]
    fn replaced_lock_is_refused_after_waiting_and_on_later_checks() {
        let _isolation = crate::test_isolation();
        let (d, c) = initialized("history-lock-replacement");
        let dir = Directory::open(d.path(), &c.cloud_id, false)
            .unwrap()
            .unwrap();
        let holder = dir.file(GATE, true, false).unwrap();
        flock(&holder, FlockOperation::NonBlockingLockExclusive).unwrap();
        let root = d.path().to_path_buf();
        let cloud_id = c.cloud_id.clone();
        let (ready, started) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let dir = Directory::open(&root, &cloud_id, false).unwrap().unwrap();
            // Admission FD must be pinned before replacement. Exercise the same
            // acquisition loop through a helper with that descriptor.
            let file = dir.file(GATE, false, false).unwrap();
            ready.send(()).unwrap();
            Lock::acquire_opened(
                &dir,
                GATE,
                file,
                false,
                Instant::now() + Duration::from_secs(2),
                &scm::Session::new(),
            )
            .is_err()
        });
        started.recv_timeout(Duration::from_secs(1)).unwrap();
        fs::rename(dir.path.join(GATE), dir.path.join("old-gate")).unwrap();
        let _ = dir.file(GATE, true, true).unwrap();
        flock(&holder, FlockOperation::Unlock).unwrap();
        assert!(thread.join().unwrap());
        let lock = Lock::acquire(
            &dir,
            GATE,
            false,
            Instant::now() + Duration::from_secs(1),
            &scm::Session::new(),
        )
        .unwrap();
        fs::rename(dir.path.join(GATE), dir.path.join("other-gate")).unwrap();
        let _ = dir.file(GATE, true, true).unwrap();
        assert!(lock.check(&dir).is_err());
    }

    /// Turn an initialized archive into an exact accepted-M4 (version 1) archive: the
    /// tables version 2 added are dropped and the header says version 1 (the result has
    /// exactly the signature `SCHEMA_V1` creates).
    fn as_version_1(d: &TestDir, c: &Cloud) -> std::path::PathBuf {
        let db = d.path().join(&c.cloud_id).join(DATABASE);
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "DROP TABLE pruned_examples; DROP TABLE archive_generation; DROP TABLE import_operations; PRAGMA user_version = 1;
             INSERT INTO collection_runs VALUES ('m4-run','sync',1,2,0,100,'complete','{}');",
        )
        .unwrap();
        let expected = Connection::open_in_memory().unwrap();
        expected.execute_batch(SCHEMA_V1).unwrap();
        assert_eq!(
            schema_signature(&conn).unwrap(),
            schema_signature(&expected).unwrap(),
            "the fixture is exactly the accepted M4 schema"
        );
        db
    }

    /// M5 review 2 finding 8: an accepted-M4 (version 1) archive is read as is, and the
    /// first writer upgrades it in place to the current schema, keeping its data; a newer
    /// or unknown schema is still refused unchanged.
    #[test]
    fn an_accepted_m4_archive_is_read_and_upgraded_in_place() {
        let _isolation = crate::test_isolation();
        let (d, c) = initialized("history-v1");
        let db = as_version_1(&d, &c);
        let r = Reader::open(d.path(), &c, &scm::Session::new())
            .unwrap()
            .unwrap();
        let g = r.generation().unwrap();
        assert_eq!(g >> 32, 0, "a version-1 archive has no incarnation yet");
        drop(r);
        let w = Writer::open(
            d.path(),
            &c,
            Duration::from_secs(10),
            false,
            &scm::Session::new(),
        )
        .unwrap();
        let (version, runs, tombstones, incarnation): (i64, i64, i64, i64) = w
            .conn
            .query_row(
                "SELECT (SELECT user_version FROM pragma_user_version), (SELECT count(*) FROM collection_runs WHERE run_id='m4-run'), (SELECT count(*) FROM pruned_examples), (SELECT incarnation FROM archive_generation)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!((version, runs, tombstones), (VERSION, 1, 0));
        assert!(incarnation > 0);
        drop(w);
        let header = fs::read(&db).unwrap();
        assert_eq!(
            u32::from_be_bytes(header[60..64].try_into().unwrap()),
            VERSION as u32
        );
        // An unknown variant of version 1 (an extra table) is refused, unchanged.
        let (d2, c2) = initialized("history-v1-unknown");
        let db2 = as_version_1(&d2, &c2);
        Connection::open(&db2)
            .unwrap()
            .execute_batch("CREATE TABLE unexpected (x INTEGER)")
            .unwrap();
        let before = fs::read(&db2).unwrap();
        assert!(
            Writer::open(
                d2.path(),
                &c2,
                Duration::from_secs(10),
                false,
                &scm::Session::new()
            )
            .is_err()
        );
        assert_eq!(fs::read(&db2).unwrap(), before);
    }

    /// M5 review 3 finding 3: the upgrade of an accepted-M4 archive happens only after
    /// a verified private copy of the version-1 file (size and SHA-256 recorded in a
    /// durable manifest) exists; a writer that must not upgrade leaves it byte for byte
    /// unchanged; the writer that upgrades records the generation it upgraded from.
    #[test]
    fn the_m4_upgrade_keeps_a_verified_pre_upgrade_copy_and_is_never_implicit_in_validation() {
        let _isolation = crate::test_isolation();
        use sha2::Digest;
        let (d, c) = initialized("history-v1-backup");
        let db = as_version_1(&d, &c);
        let original = fs::read(&db).unwrap();
        let before = {
            let r = Reader::open(d.path(), &c, &scm::Session::new())
                .unwrap()
                .unwrap();
            r.generation().unwrap()
        };
        let w = Writer::existing_unupgraded(
            d.path(),
            &c,
            Duration::from_secs(10),
            &scm::Session::new(),
        )
        .unwrap();
        drop(w);
        assert_eq!(
            fs::read(&db).unwrap(),
            original,
            "validation never upgrades"
        );
        let w =
            Writer::existing(d.path(), &c, Duration::from_secs(10), &scm::Session::new()).unwrap();
        w.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        assert!(
            w.generation().unwrap() != before,
            "the upgrade changed the generation"
        );
        assert!(
            w.bound_to(before).unwrap(),
            "only this writer's own upgrade changed it"
        );
        assert!(!w.bound_to(before + 1).unwrap());
        w.conn.execute_batch("ROLLBACK").unwrap();
        drop(w);
        let dir = d.path().join(&c.cloud_id);
        let manifests: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with(".pre-upgrade-v1.") && n.ends_with(".json"))
            .collect();
        assert_eq!(manifests.len(), 1);
        let m: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join(&manifests[0])).unwrap()).unwrap();
        let copy = fs::read(dir.join(m["database"]["file"].as_str().unwrap())).unwrap();
        assert_eq!(copy, original, "the backup is the version-1 file");
        assert_eq!(
            m["database"]["sha256"].as_str().unwrap(),
            format!("{:x}", sha2::Sha256::digest(&original))
        );
    }

    /// Child half of the version-1 hot-journal probe: an uncommitted write to an
    /// accepted-M4 archive spills a genuine hot journal, then the process exits.
    #[test]
    fn crash_v1_writer_child() {
        let _isolation = crate::test_isolation();
        let Some(path) = std::env::var_os("AIRS_HISTORY_CRASH_V1_ROOT") else {
            return;
        };
        let path = std::path::PathBuf::from(path);
        assert!(path.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        let c = cloud();
        let conn = Connection::open(path.join(&c.cloud_id).join(DATABASE)).unwrap();
        conn.pragma_update(None, "cache_size", 1).unwrap();
        conn.pragma_update(None, "cache_spill", true).unwrap();
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        for i in 0..50 {
            let id = format!("uncommitted-{i}");
            conn.execute("INSERT INTO events VALUES (?1,'app','App','session',?2,0,NULL,1,1,1,NULL,'unknown','unknown',0)",params![id,format!("00000000-0000-4000-8000-{i:012}")]).unwrap();
            let content = serde_json::json!({"padding":"x".repeat(16*1024)}).to_string();
            conn.execute(
                "INSERT INTO event_facts VALUES (?1,?1,'crash_fixture',1,1,?2)",
                params![id, content],
            )
            .unwrap();
        }
        conn.cache_flush().unwrap();
        std::process::exit(86);
    }

    /// M5 review 3 finding 3: a hot journal on an accepted-M4 (version 1) archive is
    /// recovered on the verified copy and the live file; the recovered archive is
    /// version 2 with its first incarnation, the uncommitted rows are gone, the M4 data
    /// is kept, and the raw pre-recovery copies are the backup.
    #[test]
    fn a_hot_journal_on_an_m4_archive_recovers_to_version_2() {
        let _isolation = crate::test_isolation();
        let (d, c) = initialized("history-recovery-v1");
        let _db = as_version_1(&d, &c);
        let path = d.path().join(&c.cloud_id);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "database::tests::crash_v1_writer_child",
                "--nocapture",
            ])
            .env(
                "AIRS_HISTORY_CRASH_V1_ROOT",
                d.path().canonicalize().unwrap(),
            )
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(86));
        assert!(
            path.join("archive.sqlite3-journal").exists(),
            "a real hot journal"
        );
        let result = recover(d.path(), &c, Duration::from_secs(20), &scm::Session::new()).unwrap();
        assert_eq!(result["integrity_verified"], true);
        let r = Reader::open(d.path(), &c, &scm::Session::new())
            .unwrap()
            .unwrap();
        let (version, events, runs, incarnation): (i64, i64, i64, i64) = r
            .conn
            .query_row(
                "SELECT (SELECT user_version FROM pragma_user_version), (SELECT count(*) FROM events), (SELECT count(*) FROM collection_runs WHERE run_id='m4-run'), (SELECT incarnation FROM archive_generation)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!((version, events, runs), (VERSION, 0, 1));
        assert!(incarnation > 0);
        r.finish().unwrap();
    }

    /// Review 3 minor: incarnations within an archive's lineage strictly increase (a
    /// durable counter), so a generation never repeats there; exhaustion refuses.
    #[test]
    fn incarnations_strictly_increase_and_never_wrap() {
        let first = next_incarnation(0).unwrap();
        assert!((1..=0x4000_0000).contains(&first));
        assert_eq!(next_incarnation(first).unwrap(), first + 1);
        assert!(next_incarnation(0x7fff_ffff).is_err());
    }

    /// M5 review 2 finding 1: a generation observed before a compaction never denotes
    /// different contents afterwards, however many writes follow (the counter restarts,
    /// the incarnation does not repeat).
    #[test]
    fn a_generation_never_repeats_across_compaction() {
        let _isolation = crate::test_isolation();
        let (d, c) = initialized("history-generation");
        let generation = || {
            let r = Reader::open(d.path(), &c, &scm::Session::new())
                .unwrap()
                .unwrap();
            r.generation().unwrap()
        };
        let write = |v: i64| {
            let w = Writer::open(
                d.path(),
                &c,
                Duration::from_secs(10),
                false,
                &scm::Session::new(),
            )
            .unwrap();
            w.conn
                .execute(
                    "UPDATE archive_meta SET bootstrap_ms=?1 WHERE singleton=1",
                    [v],
                )
                .unwrap();
            w.check().unwrap();
        };
        for v in 0..5 {
            write(v);
        }
        let mut seen = std::collections::BTreeSet::new();
        let preview = generation();
        seen.insert(preview);
        for round in 0..3 {
            compact(d.path(), &c, Duration::from_secs(10), &scm::Session::new()).unwrap();
            let after = generation();
            assert!(
                seen.insert(after),
                "round {round}: compaction reused a generation"
            );
            // Write well past the old counter: no generation repeats.
            for v in 0..((preview & 0xffff_ffff) + 3) {
                write(100 * (round + 1) + v);
                assert!(
                    seen.insert(generation()),
                    "round {round}: a write reused a generation"
                );
            }
        }
        assert!(!seen.is_empty());
    }

    #[test]
    fn compaction_verifies_copy_and_faults_never_publish_incomplete_data() {
        let _isolation = crate::test_isolation();
        for step in [
            None,
            Some(MaintenanceStep::BeforeCopy),
            Some(MaintenanceStep::AfterCopy),
            Some(MaintenanceStep::BeforePublish),
            Some(MaintenanceStep::AfterPublish),
            Some(MaintenanceStep::DirectorySync),
        ] {
            let (d, c) = initialized("history-compact");
            let db = d.path().join(&c.cloud_id).join(DATABASE);
            let before = fs::read(&db).unwrap();
            FAIL_MAINTENANCE.with(|v| v.set(step));
            let result = compact(d.path(), &c, Duration::from_secs(10), &scm::Session::new());
            FAIL_MAINTENANCE.with(|v| v.set(None));
            match step {
                None => {
                    assert!(result.is_ok());
                }
                Some(MaintenanceStep::AfterPublish | MaintenanceStep::DirectorySync) => {
                    let error = result.unwrap_err();
                    assert!(error.message().contains("published"));
                }
                _ => {
                    assert!(result.is_err());
                    assert_eq!(fs::read(&db).unwrap(), before);
                }
            }
            Reader::open(d.path(), &c, &scm::Session::new())
                .unwrap()
                .unwrap()
                .finish()
                .unwrap();
            assert!(!fs::read_dir(db.parent().unwrap()).unwrap().any(|e| {
                e.unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".compact.")
            }));
        }
        let d = TestDir::new("history-maint-missing");
        let c = cloud();
        assert!(compact(d.path(), &c, Duration::from_secs(1), &scm::Session::new()).is_err());
        assert!(recover(d.path(), &c, Duration::from_secs(1), &scm::Session::new()).is_err());
        assert!(fs::read_dir(d.path()).unwrap().next().is_none());
    }

    #[test]
    fn crash_writer_child() {
        let _isolation = crate::test_isolation();
        let Some(path) = std::env::var_os("AIRS_HISTORY_CRASH_TEST_ROOT") else {
            return;
        };
        let path = std::path::PathBuf::from(path);
        assert!(path.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("airs-history-recovery-")
        );
        let c = cloud();
        let w = Writer::existing(&path, &c, Duration::from_secs(10), &scm::Session::new()).unwrap();
        // Test-only spill produces a genuinely hot journal. Production batches
        // keep spill off; crashes during their commit use this same SQLite format.
        w.conn.pragma_update(None, "cache_size", 1).unwrap();
        w.conn.pragma_update(None, "cache_spill", true).unwrap();
        w.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        for i in 0..50 {
            let id = format!("uncommitted-{i}");
            w.conn.execute("INSERT INTO events VALUES (?1,'app','App','session',?2,0,NULL,1,1,1,NULL,'unknown','unknown',0)",params![id,format!("00000000-0000-4000-8000-{i:012}")]).unwrap();
            let content = serde_json::json!({"padding":"x".repeat(16*1024)}).to_string();
            w.conn
                .execute(
                    "INSERT INTO event_facts VALUES (?1,?1,'crash_fixture',1,1,?2)",
                    params![id, content],
                )
                .unwrap();
        }
        w.conn.cache_flush().unwrap();
        let journal = fs::read(w.dir.path.join("archive.sqlite3-journal")).unwrap();
        assert_eq!(
            &journal[..8],
            &[0xd9, 0xd5, 0x05, 0xf9, 0x20, 0xa1, 0x63, 0xd7]
        );
        std::process::exit(86);
    }

    #[test]
    fn hot_journal_requires_explicit_copy_verified_recovery() {
        let _isolation = crate::test_isolation();
        let (d, c) = initialized("history-recovery-parent");
        let path = d.path().join(&c.cloud_id);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "database::tests::crash_writer_child",
                "--nocapture",
            ])
            .env("AIRS_HISTORY_CRASH_TEST_ROOT", d.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(86));
        let before = state(&path);
        assert!(Reader::open(d.path(), &c, &scm::Session::new()).is_err());
        assert!(
            Writer::open(
                d.path(),
                &c,
                Duration::from_secs(1),
                false,
                &scm::Session::new()
            )
            .is_err()
        );
        assert_eq!(state(&path), before);
        let original_db = fs::read(path.join(DATABASE)).unwrap();
        let original_journal = fs::read(path.join("archive.sqlite3-journal")).unwrap();
        let result = recover(d.path(), &c, Duration::from_secs(20), &scm::Session::new()).unwrap();
        assert_eq!(result["integrity_verified"], true);
        assert_eq!(result["recovery_attempted"], true);
        let backups = &result["private_backups"];
        assert_eq!(
            fs::read(path.join(backups["database"]["file"].as_str().unwrap())).unwrap(),
            original_db
        );
        assert_eq!(
            fs::read(path.join(backups["journal"]["file"].as_str().unwrap())).unwrap(),
            original_journal
        );
        let r = Reader::open(d.path(), &c, &scm::Session::new())
            .unwrap()
            .unwrap();
        let count: i64 = r
            .conn
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        r.finish().unwrap();
        assert!(!path.join("archive.sqlite3-journal").exists());
        for e in fs::read_dir(&path).unwrap() {
            assert_eq!(e.unwrap().metadata().unwrap().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn unsupported_super_journal_and_wrong_cloud_leave_live_files_unchanged() {
        let _isolation = crate::test_isolation();
        use std::{io::Write, os::unix::fs::OpenOptionsExt};
        let (d, c) = initialized("history-recovery-refuse");
        let path = d.path().join(&c.cloud_id);
        let outside = d.path().join("outside-mj000000000");
        fs::write(&outside, b"preserve-me").unwrap();
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path.join("archive.sqlite3-journal"))
            .unwrap();
        file.write_all(&[0; 32]).unwrap();
        file.write_all(&[0xd9, 0xd5, 0x05, 0xf9, 0x20, 0xa1, 0x63, 0xd7])
            .unwrap();
        drop(file);
        let before = state(&path);
        assert!(recover(d.path(), &c, Duration::from_secs(2), &scm::Session::new()).is_err());
        assert_eq!(state(&path), before);
        assert_eq!(fs::read(outside).unwrap(), b"preserve-me");
        fs::remove_file(path.join("archive.sqlite3-journal")).unwrap();
        let conn = Connection::open(path.join(DATABASE)).unwrap();
        conn.execute("UPDATE archive_meta SET tsg_id='999'", [])
            .unwrap();
        drop(conn);
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path.join("archive.sqlite3-journal"))
            .unwrap();
        let db = fs::read(path.join(DATABASE)).unwrap();
        let journal = fs::read(path.join("archive.sqlite3-journal")).unwrap();
        assert!(recover(d.path(), &c, Duration::from_secs(5), &scm::Session::new()).is_err());
        assert_eq!(fs::read(path.join(DATABASE)).unwrap(), db);
        assert_eq!(
            fs::read(path.join("archive.sqlite3-journal")).unwrap(),
            journal
        );
    }
}
