//! Chalk-compatible styling (DESIGN §2.8): pure escape-code painting, including chalk's
//! line-break handling. Whether a stream is coloured is decided by the caller (the CLI
//! reads its environment and terminals; core never does).

/// A chalk basic style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Bold,
    Dim,
    Red,
    Green,
    Yellow,
    Cyan,
    /// chalk `blue` (the red team `RUNNING` status, DESIGN §2.8, §15.3).
    Blue,
    /// chalk `magenta` (the red team `HIGH` severity of the scan reports, §15.16).
    Magenta,
    /// chalk `white` (other red team statuses).
    White,
}

impl Style {
    fn codes(self) -> (&'static str, &'static str) {
        match self {
            Self::Bold => ("\u{1b}[1m", "\u{1b}[22m"),
            Self::Dim => ("\u{1b}[2m", "\u{1b}[22m"),
            Self::Red => ("\u{1b}[31m", "\u{1b}[39m"),
            Self::Green => ("\u{1b}[32m", "\u{1b}[39m"),
            Self::Yellow => ("\u{1b}[33m", "\u{1b}[39m"),
            Self::Cyan => ("\u{1b}[36m", "\u{1b}[39m"),
            Self::Blue => ("\u{1b}[34m", "\u{1b}[39m"),
            Self::Magenta => ("\u{1b}[35m", "\u{1b}[39m"),
            Self::White => ("\u{1b}[37m", "\u{1b}[39m"),
        }
    }
}

/// `chalk.<style>(text)` when `enabled`, else `text`. An empty string gets no codes;
/// the style is closed before every line break and reopened after it (chalk
/// `applyStyle`), so a leading newline yields `open close "\n" open …`.
pub fn paint(style: Style, text: &str, enabled: bool) -> String {
    if !enabled || text.is_empty() {
        return text.to_owned();
    }
    let (open, close) = style.codes();
    let mut out = String::with_capacity(text.len() + 10);
    out.push_str(open);
    let mut rest = text;
    while let Some(index) = rest.find('\n') {
        let (line, break_len) = match rest[..index].strip_suffix('\r') {
            Some(line) => (line, 2),
            None => (&rest[..index], 1),
        };
        out.push_str(line);
        out.push_str(close);
        out.push_str(if break_len == 2 { "\r\n" } else { "\n" });
        out.push_str(open);
        rest = &rest[index + 1..];
    }
    out.push_str(rest);
    out.push_str(close);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chalk_codes_and_line_breaks() {
        assert_eq!(paint(Style::Bold, "t", true), "\u{1b}[1mt\u{1b}[22m");
        assert_eq!(paint(Style::Dim, "t", true), "\u{1b}[2mt\u{1b}[22m");
        assert_eq!(paint(Style::Red, "t", true), "\u{1b}[31mt\u{1b}[39m");
        assert_eq!(paint(Style::Green, "t", true), "\u{1b}[32mt\u{1b}[39m");
        assert_eq!(paint(Style::Yellow, "t", true), "\u{1b}[33mt\u{1b}[39m");
        assert_eq!(paint(Style::Cyan, "t", true), "\u{1b}[36mt\u{1b}[39m");
        assert_eq!(paint(Style::Blue, "t", true), "\u{1b}[34mt\u{1b}[39m");
        assert_eq!(paint(Style::Magenta, "t", true), "\u{1b}[35mt\u{1b}[39m");
        assert_eq!(paint(Style::White, "t", true), "\u{1b}[37mt\u{1b}[39m");
        assert_eq!(paint(Style::Red, "", true), "");
        assert_eq!(paint(Style::Red, "t", false), "t");
        // chalk.red("\n  ✗ Error: x")
        assert_eq!(
            paint(Style::Red, "\n  ✗ Error: x", true),
            "\u{1b}[31m\u{1b}[39m\n\u{1b}[31m  ✗ Error: x\u{1b}[39m"
        );
        // chalk.red("\n  ✗ m\n")
        assert_eq!(
            paint(Style::Red, "\n  ✗ m\n", true),
            "\u{1b}[31m\u{1b}[39m\n\u{1b}[31m  ✗ m\u{1b}[39m\n\u{1b}[31m\u{1b}[39m"
        );
        assert_eq!(
            paint(Style::Dim, "a\r\nb", true),
            "\u{1b}[2ma\u{1b}[22m\r\n\u{1b}[2mb\u{1b}[22m"
        );
    }
}
