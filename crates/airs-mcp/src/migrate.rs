//! Migration of an existing local development archive into the service's storage
//! (plan "History transfer and lifecycle", M5.2; contract §5.1), through the explicit
//! import workflow. Offline maintenance of the stopped service
//! (`airs-mcp migrate-archive --config PATH --source DIR`):
//!
//! 1. the source is the recognized archive layout `<DIR>/<cloud_id>/archive.sqlite3`:
//!    a regular single file owned by this user, with no journal/WAL sidecars, at most
//!    16 GiB; it is opened read-only and never modified (its SHA-256 is checked before
//!    the copy and again at the end);
//! 2. a bounded private owned copy is staged under the service's data subtree and
//!    verified against the source hash;
//! 3. the staged copy is exported as a logical transfer: the store's own admission
//!    validates its schema (an unknown or newer schema refuses) and its cloud identity;
//! 4. a verified rollback copy of the destination archive is kept when one exists;
//! 5. the transfer is imported through the full staged validation of `import_file`.
//!
//! The CLI never opens SQLite; this runs inside the service binary.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use airs_history_store::identity::Cloud;
use airs_runtime_core::error::SecretSet;

const DATABASE: &str = "archive.sqlite3";
const MAX_BYTES: u64 = 17_179_869_184;
const BUDGET: Duration = Duration::from_secs(600);

fn hash_file(path: &Path) -> Result<(String, u64), &'static str> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)
        .map_err(|_| "cannot open the archive read-only")?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut total = 0u64;
    loop {
        let n = f.read(&mut buf).map_err(|_| "cannot read the archive")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
    Ok((format!("{:x}", hasher.finalize()), total))
}

/// The admitted source archive file: regular, single-link, owned by this user, no
/// sidecars, within the transfer ceiling.
fn admit_source(dir: &Path, cloud: &Cloud) -> Result<PathBuf, &'static str> {
    use std::os::unix::fs::MetadataExt;
    let archive = dir.join(&cloud.cloud_id);
    let file = archive.join(DATABASE);
    let meta = std::fs::symlink_metadata(&file)
        .map_err(|_| "no recognized archive for this service's cloud in the source")?;
    if !meta.file_type().is_file()
        || meta.nlink() != 1
        || meta.uid() != rustix::process::geteuid().as_raw()
        || meta.len() > MAX_BYTES
    {
        return Err("the source archive is not a regular, single, owned file within 16 GiB");
    }
    for suffix in ["-wal", "-shm", "-journal"] {
        if std::fs::symlink_metadata(archive.join(format!("{DATABASE}{suffix}"))).is_ok() {
            return Err("the source archive has a journal or WAL sidecar; it was left unchanged");
        }
    }
    Ok(file)
}

/// Copy `from` to a new private file `to` (0600) and verify its SHA-256.
fn verified_copy(from: &Path, to: &Path, expected: &str) -> Result<(), &'static str> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let parent = to.parent().ok_or("invalid staging path")?;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
        .map_err(|_| "cannot create private staging")?;
    let mut source = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(from)
        .map_err(|_| "cannot open the archive read-only")?;
    let mut target = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(to)
        .map_err(|_| "cannot create the private copy")?;
    std::io::copy(&mut source, &mut target).map_err(|_| "cannot copy the archive")?;
    target
        .sync_all()
        .map_err(|_| "cannot sync the private copy")?;
    if hash_file(to)?.0 != expected {
        return Err("the private copy does not match the source");
    }
    Ok(())
}

struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Migrate the archive of `cloud` found under `source` into `history_root` (the
/// service's history directory), staging under `data_root`.
pub fn migrate_archive(
    data_root: &Path,
    history_root: &Path,
    source: &Path,
    cloud: &Cloud,
    secrets: &SecretSet,
) -> Result<Value, &'static str> {
    let file = admit_source(source, cloud)?;
    let (source_sha, source_bytes) = hash_file(&file)?;
    let staging = data_root
        .join("staging")
        .join(format!("migrate-{}", uuid::Uuid::new_v4()));
    let _cleanup = Cleanup(staging.clone());
    let staged_root = staging.join("history");
    verified_copy(
        &file,
        &staged_root.join(&cloud.cloud_id).join(DATABASE),
        &source_sha,
    )?;
    // The store's own cooperative lock files for the staged copy (fresh, empty and
    // private: nothing else is copied from the source).
    for lock in ["maintenance.lock", "writer.lock"] {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(staged_root.join(&cloud.cloud_id).join(lock))
            .map_err(|_| "cannot prepare the private copy")?;
    }
    let session = scm::Session::new();
    let transfer = staging.join("transfer.jsonl");
    let exported = airs_history_store::transfer::export_file(
        &staged_root,
        cloud,
        &transfer,
        BUDGET,
        &session,
        secrets,
    )
    .map_err(
        |_| "the source archive's schema or cloud identity is not recognized; nothing was imported",
    )?;
    // A verified rollback copy of the destination, if there is one.
    let destination = history_root.join(&cloud.cloud_id).join(DATABASE);
    let rollback = if std::fs::symlink_metadata(&destination).is_ok() {
        let (sha, _) = hash_file(&destination)?;
        let backup = data_root
            .join("backups")
            .join(format!("pre-migrate-{}", airs_runtime_core::time::now_ms()))
            .join(&cloud.cloud_id)
            .join(DATABASE);
        verified_copy(&destination, &backup, &sha)?;
        Some(json!({"path": backup.display().to_string(), "sha256": sha}))
    } else {
        None
    };
    let imported = airs_history_store::transfer::import_file(
        history_root,
        cloud,
        &transfer,
        BUDGET,
        &session,
        secrets,
    )
    .map_err(
        |_| "the import was refused; the destination is unchanged or completed idempotently",
    )?;
    if hash_file(&file)?.0 != source_sha {
        return Err("the source archive changed during the migration");
    }
    Ok(json!({
        "event": "archive_migrated",
        "cloud_id": cloud.cloud_id,
        "source_sha256": source_sha,
        "source_bytes": source_bytes,
        "transfer_sha256": exported["sha256"],
        "records": imported["records"],
        "inserted": imported["inserted"],
        "rollback": rollback,
        "source_unchanged": true,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::DirBuilderExt;

    fn dir(tag: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("airs-mcp-migrate-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new().mode(0o700).create(&d).unwrap();
        d.canonicalize().unwrap()
    }

    fn cloud() -> Cloud {
        Cloud::from_parts(
            "1234567890",
            [
                "https://api.example.test/aisec".into(),
                "https://api.example.test/aisec".into(),
                "https://auth.example.test/oauth2/access_token".into(),
            ],
        )
        .unwrap()
    }

    fn seed(root: &Path, collection: &str) {
        let mut v: Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/signal/evidence.json"))
                .unwrap();
        v["collection_id"] = json!(collection);
        v.as_object_mut().unwrap().remove("digest");
        v["digest"] = json!(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&v).unwrap())
        ));
        let e = airs_runtime_core::signal::evidence::Evidence::parse(&v.to_string()).unwrap();
        airs_history_store::repository::import_evidence(
            root,
            &cloud(),
            &e,
            &SecretSet::default(),
            &Default::default(),
            &scm::Session::new(),
        )
        .unwrap();
    }

    fn events(history: &Path) -> i64 {
        rusqlite::Connection::open_with_flags(
            history.join(cloud().cloud_id).join(DATABASE),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap()
        .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
        .unwrap()
    }

    /// An existing development archive migrates through the import workflow: the
    /// source is untouched, the staging removed, a verified rollback copy kept of an
    /// existing destination; a repeat is idempotent.
    #[test]
    fn a_development_archive_migrates_and_its_original_is_untouched() {
        let base = dir("ok");
        let source = base.join("old-history");
        seed(&source, "11111111-1111-4111-8111-111111111111");
        let original = std::fs::read(source.join(cloud().cloud_id).join(DATABASE)).unwrap();
        let data = base.join("data");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&data)
            .unwrap();
        let history = data.join("history");
        seed(&history, "22222222-2222-4222-8222-222222222222");
        let r = migrate_archive(&data, &history, &source, &cloud(), &SecretSet::default()).unwrap();
        assert!(r["inserted"].as_u64().unwrap() > 0, "{r}");
        assert!(
            r["rollback"]["sha256"].is_string(),
            "a rollback copy is kept: {r}"
        );
        assert_eq!(events(&history), 1);
        assert_eq!(
            std::fs::read(source.join(cloud().cloud_id).join(DATABASE)).unwrap(),
            original,
            "the original is never modified"
        );
        assert!(
            std::fs::read_dir(data.join("staging"))
                .unwrap()
                .next()
                .is_none(),
            "the staging is removed"
        );
        let again =
            migrate_archive(&data, &history, &source, &cloud(), &SecretSet::default()).unwrap();
        assert_eq!(again["inserted"], 0, "a repeat is idempotent");
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// M5 review 2 finding 8: a source with exactly the accepted-M4 (version 1) schema
    /// (the frozen `SCHEMA_V1`) migrates: its private copy is recognized and upgraded,
    /// its data imported, and the original left untouched.
    #[test]
    fn an_accepted_m4_archive_migrates() {
        let base = dir("m4");
        let source = base.join("old-history");
        seed(&source, "44444444-4444-4444-8444-444444444444");
        let file = source.join(cloud().cloud_id).join(DATABASE);
        {
            let conn = rusqlite::Connection::open(&file).unwrap();
            conn.execute_batch(
                "DROP TABLE pruned_examples; DROP TABLE archive_generation; DROP TABLE import_operations; PRAGMA user_version = 1;",
            )
            .unwrap();
            let expected = rusqlite::Connection::open_in_memory().unwrap();
            expected
                .execute_batch(airs_history_store::SCHEMA_V1)
                .unwrap();
            let signature = |c: &rusqlite::Connection| -> Vec<(String, String, String)> {
                c.prepare("SELECT type,name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name")
                    .unwrap()
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                    .unwrap()
                    .collect::<rusqlite::Result<_>>()
                    .unwrap()
            };
            assert_eq!(
                signature(&conn),
                signature(&expected),
                "exactly the M4 schema"
            );
        }
        let original = std::fs::read(&file).unwrap();
        let data = base.join("data");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&data)
            .unwrap();
        let history = data.join("history");
        let r = migrate_archive(&data, &history, &source, &cloud(), &SecretSet::default()).unwrap();
        assert!(r["inserted"].as_u64().unwrap() > 0, "{r}");
        assert_eq!(events(&history), 1);
        assert_eq!(
            std::fs::read(&file).unwrap(),
            original,
            "the original is untouched"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// An unknown (newer) schema, a missing archive for this cloud and a sidecar are
    /// refused, and the destination is untouched.
    #[test]
    fn an_unrecognized_source_is_refused_without_changes() {
        let base = dir("refused");
        let source = base.join("old-history");
        seed(&source, "33333333-3333-4333-8333-333333333333");
        let file = source.join(cloud().cloud_id).join(DATABASE);
        let data = base.join("data");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&data)
            .unwrap();
        let history = data.join("history");
        // A newer schema (the header's user_version).
        let mut bytes = std::fs::read(&file).unwrap();
        let good = bytes.clone();
        bytes[60..64].copy_from_slice(&99u32.to_be_bytes());
        std::fs::write(&file, &bytes).unwrap();
        assert!(
            migrate_archive(&data, &history, &source, &cloud(), &SecretSet::default()).is_err()
        );
        assert!(!history.exists(), "nothing was imported");
        std::fs::write(&file, &good).unwrap();
        // A sidecar.
        std::fs::write(
            source
                .join(cloud().cloud_id)
                .join("archive.sqlite3-journal"),
            b"x",
        )
        .unwrap();
        assert!(
            migrate_archive(&data, &history, &source, &cloud(), &SecretSet::default()).is_err()
        );
        // No archive of this cloud.
        let empty = base.join("empty");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&empty)
            .unwrap();
        assert!(migrate_archive(&data, &history, &empty, &cloud(), &SecretSet::default()).is_err());
        assert!(!history.exists());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
