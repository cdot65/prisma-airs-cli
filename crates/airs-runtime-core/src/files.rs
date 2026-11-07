//! Bounded regular-file reads (DESIGN §13.2, D103): fixed messages that name the path
//! as typed and echo neither the OS error nor any file content.

use crate::error::CliError;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// The path as the messages show it (lossy for non-UTF-8 bytes).
pub fn shown(path: &OsStr) -> String {
    path.to_string_lossy().into_owned()
}

fn unreadable(path: &OsStr, limit: u64) -> CliError {
    CliError::operational(format!(
        "Cannot read {} (missing, not a regular file, unreadable or larger than {} MiB)",
        shown(path),
        limit / 1_048_576
    ))
}

/// Read the whole regular file at `path` (symlinks followed) of at most `limit` bytes.
pub fn read_bytes(path: &OsStr, limit: u64) -> Result<Vec<u8>, CliError> {
    let fail = || unreadable(path, limit);
    let target = Path::new(path);
    // A FIFO or device is refused before it is opened (opening a FIFO blocks).
    let metadata = std::fs::metadata(target).map_err(|_| fail())?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(fail());
    }
    let file = File::open(target).map_err(|_| fail())?;
    // The file that was opened must still be a regular file within the limit.
    let opened = file.metadata().map_err(|_| fail())?;
    if !opened.is_file() || opened.len() > limit {
        return Err(fail());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| fail())?;
    if bytes.len() as u64 > limit {
        return Err(fail());
    }
    Ok(bytes)
}

/// Read the whole file at `path` as UTF-8 text of at most `limit` bytes.
pub fn read_text(path: &OsStr, limit: u64) -> Result<String, CliError> {
    String::from_utf8(read_bytes(path, limit)?)
        .map_err(|_| CliError::operational(format!("{} is not valid UTF-8", shown(path))))
}

/// Whether the last component of `path` names a file: not empty (a trailing `/`), `.`
/// or `..` (D226). `lstat("dir/")` of a missing `dir` is "not found" and its parent is
/// the working directory, so such a path would pass every other check and fail only at
/// the final `link`, after the request.
pub fn names_a_file(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let bytes = path.as_os_str().as_bytes();
    let last = bytes
        .rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or_default();
    !last.is_empty() && last != b"." && last != b".."
}
