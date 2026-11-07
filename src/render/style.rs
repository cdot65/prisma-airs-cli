//! The colour decision for this process's streams (DESIGN §2.8): clap's own `anstream`
//! auto rule (via the same `anstyle-query` reads), made once per process for stdout and,
//! independently, for stderr (D58). The chalk-compatible styling itself ([`Style`],
//! [`paint`]) is pure and lives in airs-runtime-core.

pub use airs_runtime_core::style::{Style, paint};
use std::ffi::OsString;
use std::io::IsTerminal;
use std::sync::OnceLock;

/// The §2.8 decision for one stream, from an environment lookup and whether the stream
/// is a terminal (`anstream::AutoStream` auto choice).
pub fn decide(var: &dyn Fn(&str) -> Option<OsString>, is_terminal: bool) -> bool {
    let non_empty = |name: &str| var(name).is_some_and(|value| !value.is_empty());
    let clicolor = var("CLICOLOR").map(|value| value != "0");
    if non_empty("NO_COLOR") {
        false
    } else if non_empty("CLICOLOR_FORCE") {
        true
    } else if clicolor == Some(false) {
        false
    } else {
        let term = var("TERM").is_some_and(|term| term != "dumb");
        is_terminal && (term || clicolor == Some(true) || var("CI").is_some())
    }
}

fn process_decision(is_terminal: bool) -> bool {
    decide(&|name| std::env::var_os(name), is_terminal)
}

/// Whether stdout is coloured in this process (decided once).
pub fn stdout_color() -> bool {
    static DECISION: OnceLock<bool> = OnceLock::new();
    *DECISION.get_or_init(|| process_decision(std::io::stdout().is_terminal()))
}

/// Whether stderr is coloured in this process (decided once).
pub fn stderr_color() -> bool {
    static DECISION: OnceLock<bool> = OnceLock::new();
    *DECISION.get_or_init(|| process_decision(std::io::stderr().is_terminal()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(vars: &[(&str, &str)], tty: bool) -> bool {
        let vars: Vec<(String, OsString)> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), OsString::from(v)))
            .collect();
        decide(
            &|name| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()),
            tty,
        )
    }

    #[test]
    fn decision_matches_the_clap_rule() {
        // A terminal needs TERM (not dumb), CLICOLOR or CI.
        assert!(with(&[("TERM", "xterm-256color")], true));
        assert!(!with(&[("TERM", "xterm-256color")], false));
        assert!(!with(&[], true));
        assert!(!with(&[("TERM", "dumb")], true));
        assert!(with(&[("CLICOLOR", "1")], true));
        assert!(with(&[("CI", "")], true));
        assert!(!with(&[("CI", "1")], false));
        // NO_COLOR (non-empty) wins over everything.
        assert!(!with(
            &[
                ("NO_COLOR", "1"),
                ("CLICOLOR_FORCE", "1"),
                ("TERM", "xterm")
            ],
            true
        ));
        assert!(with(&[("NO_COLOR", ""), ("TERM", "xterm")], true));
        // CLICOLOR_FORCE (any non-empty value, 0 included) forces colour on a pipe.
        assert!(with(&[("CLICOLOR_FORCE", "1")], false));
        assert!(with(&[("CLICOLOR_FORCE", "0")], false));
        assert!(!with(&[("CLICOLOR_FORCE", "")], false));
        // CLICOLOR=0 disables on a terminal.
        assert!(!with(&[("CLICOLOR", "0"), ("TERM", "xterm")], true));
        // FORCE_COLOR is not read (D58).
        assert!(!with(&[("FORCE_COLOR", "1")], false));
        assert!(!with(&[("FORCE_COLOR", "3")], true));
    }
}
