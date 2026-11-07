//! Durable operation receipts (contract §7.4): created before the first submission
//! send, updated with the job ID, removed only after the job is terminal and its result
//! delivered. No token, no payload. `receipts/<connection>/<idempotency_key>.json`,
//! 0600, ≤4 KiB, ≤1,000 per connection.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const MAX_RECEIPT_BYTES: u64 = 4096;

/// Version 2 receipts are written only after the server's job ID was checked against
/// the writing invocation's secrets (M7 review 3). A version-1 receipt predates that
/// check: its job ID can carry a token in a valid shape, and no later invocation can
/// tell. It is kept (it records an unresolved submission and its key), but its
/// server-derived job ID is never shown (M7 review 6).
pub const FORMAT: &str = "airs-mcp-op/2";
pub const MAX_RECEIPTS: usize = 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub format: String,
    pub operation: String,
    #[serde(rename = "paramsDigest")]
    pub params_digest: String,
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: String,
    pub epoch: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "jobId")]
    pub job_id: Option<String>,
    /// The connection revision the key was minted under; a changed connection leaves
    /// the receipt listed as unresolved, never silently replaced.
    #[serde(rename = "connectionRevision")]
    pub connection_revision: String,
}

impl Receipt {
    /// The job ID, only from a receipt written under the secret guard.
    pub fn trusted_job_id(&self) -> Option<&str> {
        (self.format == FORMAT)
            .then_some(self.job_id.as_deref())
            .flatten()
    }

    /// The idempotency key, when it has the CLI's own form (`cli-` and 32 lowercase hex).
    pub fn shown_key(&self) -> Option<&str> {
        let k = self.idempotency_key.strip_prefix("cli-")?;
        (k.len() == 32
            && k.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        .then_some(self.idempotency_key.as_str())
    }

    /// The operation, when it is a tool name (`[a-z_]`, at most 32).
    pub fn shown_operation(&self) -> Option<&str> {
        let o = self.operation.as_str();
        ((1..=32).contains(&o.len()) && o.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
            .then_some(o)
    }
}

pub struct Receipts {
    dir: PathBuf,
}

pub(crate) fn private(dir: &Path) -> Result<(), &'static str> {
    match fs::symlink_metadata(dir) {
        Ok(m) => {
            if !m.is_dir()
                || m.uid() != rustix::process::geteuid().as_raw()
                || m.mode() & 0o077 != 0
            {
                return Err("receipt directory must be a private 0700 directory owned by you");
            }
            Ok(())
        }
        Err(_) => fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .map_err(|_| "cannot create the receipt directory"),
    }
}

fn valid_key(key: &str) -> bool {
    (16..=64).contains(&key.len())
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

impl Receipts {
    pub fn open(root: &Path, connection: &str) -> Result<Self, &'static str> {
        let base = root.join("receipts");
        private(root)?;
        private(&base)?;
        let dir = base.join(connection);
        private(&dir)?;
        Ok(Self { dir })
    }

    fn path(&self, key: &str) -> Result<PathBuf, &'static str> {
        if !valid_key(key) {
            return Err("invalid idempotency key");
        }
        Ok(self.dir.join(format!("{key}.json")))
    }

    /// Write `receipt` atomically (temp + fsync + rename + directory fsync).
    pub fn put(&self, receipt: &Receipt) -> Result<(), &'static str> {
        let path = self.path(&receipt.idempotency_key)?;
        if !path.exists() && self.list()?.len() >= MAX_RECEIPTS {
            return Err("too many unresolved operation receipts for this connection");
        }
        let bytes = serde_json::to_vec(receipt).map_err(|_| "cannot encode the receipt")?;
        if bytes.len() as u64 > MAX_RECEIPT_BYTES {
            return Err("the receipt exceeds its bound");
        }
        let tmp = self.dir.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|_| "cannot write the receipt")?;
        f.write_all(&bytes)
            .and_then(|_| f.sync_all())
            .map_err(|_| "cannot write the receipt")?;
        drop(f);
        fs::rename(&tmp, &path).map_err(|_| "cannot publish the receipt")?;
        fs::File::open(&self.dir)
            .and_then(|d| d.sync_all())
            .map_err(|_| "cannot sync the receipt directory")
    }

    pub fn remove(&self, key: &str) -> Result<(), &'static str> {
        match fs::remove_file(self.path(key)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("cannot remove the receipt"),
        }
    }

    /// Every receipt (unresolved operations), bounded reads.
    pub fn list(&self) -> Result<Vec<Receipt>, &'static str> {
        self.inventory().map(|(list, _)| list)
    }

    /// Every readable receipt, and how many receipt files could not be read or are not
    /// receipts (they are kept, never removed): a caller that reports unresolved work
    /// must not present a partial inventory as complete (M7 review 7 finding 2).
    pub fn inventory(&self) -> Result<(Vec<Receipt>, usize), &'static str> {
        let mut unreadable = 0;
        let mut out = Vec::new();
        for entry in fs::read_dir(&self.dir).map_err(|_| "cannot list receipts")? {
            let entry = entry.map_err(|_| "cannot list receipts")?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.starts_with('.') || !name.ends_with(".json") {
                continue;
            }
            let m = fs::symlink_metadata(entry.path()).map_err(|_| "cannot read a receipt")?;
            if !m.is_file() || m.len() > MAX_RECEIPT_BYTES {
                unreadable += 1;
                continue;
            }
            match fs::read(entry.path())
                .ok()
                .and_then(|b| serde_json::from_slice::<Receipt>(&b).ok())
            {
                Some(r) => out.push(r),
                None => unreadable += 1,
            }
        }
        out.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then(a.idempotency_key.cmp(&b.idempotency_key))
        });
        Ok((out, unreadable))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipts_are_private_atomic_bounded_and_listed() {
        let root = std::env::temp_dir().join(format!("airs-receipts-{}", uuid::Uuid::new_v4()));
        let r = Receipts::open(&root, "goshawks").unwrap();
        let mut receipt = Receipt {
            format: FORMAT.into(),
            operation: "collection_start".into(),
            params_digest: "a".repeat(64),
            idempotency_key: "key-0123456789abcdef".into(),
            epoch: "0a1b2c3d".into(),
            created_at: 1,
            job_id: None,
            connection_revision: "00000000-0000-4000-8000-0000000000aa".into(),
        };
        r.put(&receipt).unwrap();
        receipt.job_id = Some("j1_x".into());
        r.put(&receipt).unwrap();
        assert_eq!(r.list().unwrap(), vec![receipt.clone()]);
        let mode = fs::metadata(root.join("receipts/goshawks/key-0123456789abcdef.json"))
            .unwrap()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        assert!(
            r.put(&Receipt {
                idempotency_key: "../x".into(),
                ..receipt.clone()
            })
            .is_err()
        );
        r.remove(&receipt.idempotency_key).unwrap();
        assert!(r.list().unwrap().is_empty());
        fs::remove_dir_all(&root).unwrap();
    }
}
