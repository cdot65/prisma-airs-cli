//! Unit-test helpers shared across the crate's test modules (compiled for tests only).

use std::path::{Path, PathBuf};

/// A fresh directory under the system temp directory, removed when dropped.
pub struct TempDir(PathBuf);

impl TempDir {
    /// A new empty directory named `airs-<label>-<random>`.
    pub fn new(label: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("airs-{label}-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// `name` inside the directory.
    pub fn join(&self, name: impl AsRef<Path>) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
