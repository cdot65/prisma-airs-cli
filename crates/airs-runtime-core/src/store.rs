//! Filesystem and TOML primitives shared by the CLI tenant store and the server
//! (DESIGN §3): lexical paths, the effective user, and the strict TOML forms.

use std::path::{Component, Path, PathBuf};

/// The program name every usage line, hint, remedy and success line uses (D73, D326):
/// clap's `name` and `bin_name`, so it never depends on `argv[0]` (DESIGN §21.2).
pub const PROGRAM: &str = "airs";

/// The largest `config.toml` or tenant file read or written, in bytes.
pub const MAX_FILE_BYTES: u64 = 1_048_576;

/// Lexical normalisation like Node's `path.resolve`/`path.join`: drops `.`, resolves
/// `..` (never above the root), duplicate and trailing separators.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    let mut depth = 0usize;
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if depth > 0 {
                    out.pop();
                    depth -= 1;
                } else if !out.has_root() {
                    out.push("..");
                }
            }
            Component::Normal(name) => {
                out.push(name);
                depth += 1;
            }
        }
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

/// `^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$`
pub fn valid_tenant_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
}

/// The effective user id.
pub fn euid() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// A TOML basic string (§3.2): `"` and `\` escaped, the five short escapes, every other
/// C0 control, DEL and C1 control as `\uXXXX`, everything else raw UTF-8.
pub fn toml_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Parse TOML text into a table; any failure (the `toml` crate's message quotes the
/// input) is `None`, so nothing from the file can reach a message.
pub fn parse_toml(bytes: &[u8]) -> Option<toml::Table> {
    let text = std::str::from_utf8(bytes).ok()?;
    text.parse::<toml::Table>().ok()
}
