//! Private lifecycle preview records (contract §3, §7.4): what `lifecycle_preview`
//! returned, so `--execute PREVIEW_ID` and `airs mcp confirm PREVIEW_ID` bind the exact
//! digest and proof nonce the service issued. A record authorizes nothing on its own:
//! the nonce is accepted only inside a fresh, gateway-signed administrative sign-in.
//! `previews/<connection>/<preview_id>.json`, 0600, ≤4 KiB, ≤100 per connection.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::receipts::private;

pub const MAX_PREVIEW_BYTES: u64 = 4096;
pub const MAX_PREVIEWS: usize = 100;
/// Version 2 records are written only after every field was checked against the
/// invocation's secrets (M7 review 5). Version 1 records predate that check: a field
/// can carry a token in a valid shape (a UUID inside the preview ID), and no later
/// invocation can tell, so they are never read; the preview is simply recreated.
pub const FORMAT: &str = "airs-mcp-preview/2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewRecord {
    pub format: String,
    #[serde(rename = "previewId")]
    pub preview_id: String,
    pub action: String,
    #[serde(rename = "parameterDigest")]
    pub parameter_digest: String,
    #[serde(rename = "proofNonce")]
    pub proof_nonce: Option<String>,
    #[serde(rename = "expiresAt")]
    pub expires_at: String,
    #[serde(rename = "connectionRevision")]
    pub connection_revision: String,
}

/// `p1_<8 hex>_<uuidv4>` (contract §4.2), so a record name is never a path.
pub fn valid_preview_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("p1_") else {
        return false;
    };
    let Some((epoch, uuid)) = rest.split_once('_') else {
        return false;
    };
    epoch.len() == 8
        && epoch
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        && uuid.len() == 36
        && uuid::Uuid::parse_str(uuid).is_ok_and(|u| u.get_version_num() == 4)
        && uuid == uuid.to_ascii_lowercase()
}

/// Whether every field of a preview record has its contract form: the preview ID, an
/// action name (`[a-z_]`, at most 32), a 64-hex parameter digest, an optional 32-hex
/// proof nonce and an RFC 3339 UTC expiry. A record that fails is never written, and
/// one read back that fails is never shown or used. A later invocation does not know
/// an earlier invocation's tokens, so the form itself is the guard (M7 review 4).
pub fn well_formed(r: &PreviewRecord) -> bool {
    let hex = |s: &str, n: usize| {
        s.len() == n
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    let utc = |s: &str| {
        let b = s.as_bytes();
        let digits = |r: std::ops::Range<usize>| r.into_iter().all(|i| b[i].is_ascii_digit());
        (20..=30).contains(&b.len())
            && digits(0..4)
            && b[4] == b'-'
            && digits(5..7)
            && b[7] == b'-'
            && digits(8..10)
            && b[10] == b'T'
            && digits(11..13)
            && b[13] == b':'
            && digits(14..16)
            && b[16] == b':'
            && digits(17..19)
            && b[b.len() - 1] == b'Z'
            && (b.len() == 20 || (b[19] == b'.' && b.len() > 21 && digits(20..b.len() - 1)))
    };
    r.format == FORMAT
        && valid_preview_id(&r.preview_id)
        && (1..=32).contains(&r.action.len())
        && r.action
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'_')
        && hex(&r.parameter_digest, 64)
        && r.proof_nonce.as_deref().is_none_or(|n| hex(n, 32))
        && utc(&r.expires_at)
}

pub struct Previews {
    dir: PathBuf,
}

impl Previews {
    pub fn open(root: &Path, connection: &str) -> Result<Self, &'static str> {
        let base = root.join("previews");
        private(root)?;
        private(&base)?;
        let dir = base.join(connection);
        private(&dir)?;
        Ok(Self { dir })
    }

    fn path(&self, id: &str) -> Result<PathBuf, &'static str> {
        if !valid_preview_id(id) {
            return Err("invalid preview ID");
        }
        Ok(self.dir.join(format!("{id}.json")))
    }

    /// Write `record` atomically (temp + fsync + rename + directory fsync); the oldest
    /// records beyond the bound are removed first (previews expire within minutes).
    pub fn put(&self, record: &PreviewRecord) -> Result<(), &'static str> {
        let path = self.path(&record.preview_id)?;
        let mut names = self.names()?;
        names.sort();
        while names.len() >= MAX_PREVIEWS {
            let oldest = names.remove(0);
            let _ = fs::remove_file(self.dir.join(oldest));
        }
        let bytes = serde_json::to_vec(record).map_err(|_| "cannot encode the preview record")?;
        if bytes.len() as u64 > MAX_PREVIEW_BYTES {
            return Err("the preview record exceeds its bound");
        }
        let tmp = self.dir.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|_| "cannot write the preview record")?;
        f.write_all(&bytes)
            .and_then(|_| f.sync_all())
            .map_err(|_| "cannot write the preview record")?;
        drop(f);
        fs::rename(&tmp, &path).map_err(|_| "cannot publish the preview record")?;
        fs::File::open(&self.dir)
            .and_then(|d| d.sync_all())
            .map_err(|_| "cannot sync the preview directory")
    }

    /// The record of `id`, read through a private regular file (bounded).
    pub fn get(&self, id: &str) -> Result<Option<PreviewRecord>, &'static str> {
        let path = self.path(id)?;
        let m = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };
        if !m.is_file()
            || m.uid() != rustix::process::geteuid().as_raw()
            || m.mode() & 0o077 != 0
            || m.nlink() != 1
            || m.len() > MAX_PREVIEW_BYTES
        {
            return Err("the preview record is not private");
        }
        let r: PreviewRecord =
            serde_json::from_slice(&fs::read(&path).map_err(|_| "cannot read the preview record")?)
                .map_err(|_| "the preview record is invalid")?;
        Ok((r.format == FORMAT && r.preview_id == id).then_some(r))
    }

    pub fn remove(&self, id: &str) -> Result<(), &'static str> {
        match fs::remove_file(self.path(id)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("cannot remove the preview record"),
        }
    }

    fn names(&self) -> Result<Vec<String>, &'static str> {
        let mut out = Vec::new();
        for entry in fs::read_dir(&self.dir).map_err(|_| "cannot list preview records")? {
            let entry = entry.map_err(|_| "cannot list preview records")?;
            if let Some(name) = entry.file_name().to_str()
                && !name.starts_with('.')
                && name.ends_with(".json")
            {
                out.push(name.to_owned());
            }
        }
        Ok(out)
    }
}

/// The record of `id` under any connection of this state root (an agent-prepared
/// preview is confirmed through another, admin-role, connection).
pub fn find(root: &Path, id: &str) -> Result<Option<PreviewRecord>, &'static str> {
    if !valid_preview_id(id) {
        return Err("invalid preview ID");
    }
    let base = root.join("previews");
    let Ok(entries) = fs::read_dir(&base) else {
        return Ok(None);
    };
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !airs_runtime_core::tenant::mcp::valid_name(&name) {
            continue;
        }
        if let Some(r) = Previews::open(root, &name)?.get(id)? {
            return Ok(Some(r));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_records_are_private_named_by_their_id_and_found_across_connections() {
        let root = std::env::temp_dir().join(format!("airs-previews-{}", uuid::Uuid::new_v4()));
        let id = "p1_0a1b2c3d_9f0e8d7c-6b5a-4f3e-8d2c-1b0a9f8e7d6c";
        let record = PreviewRecord {
            format: FORMAT.into(),
            preview_id: id.into(),
            action: "prune".into(),
            parameter_digest: "a".repeat(64),
            proof_nonce: Some("b".repeat(32)),
            expires_at: "2026-11-06T00:05:00Z".into(),
            connection_revision: "00000000-0000-4000-8000-0000000000aa".into(),
        };
        let p = Previews::open(&root, "agent").unwrap();
        p.put(&record).unwrap();
        assert_eq!(p.get(id).unwrap(), Some(record.clone()));
        assert_eq!(find(&root, id).unwrap(), Some(record.clone()));
        let mode = fs::metadata(root.join(format!("previews/agent/{id}.json")))
            .unwrap()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        for bad in [
            "../x",
            "p1_0A1B2C3D_9f0e8d7c-6b5a-4f3e-8d2c-1b0a9f8e7d6c",
            "p1_x",
        ] {
            assert!(p.get(bad).is_err(), "{bad}");
        }
        p.remove(id).unwrap();
        assert_eq!(find(&root, id).unwrap(), None);
        fs::remove_dir_all(&root).unwrap();
    }
}
