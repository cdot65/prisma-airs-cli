//! The bulk-scan input file (DESIGN §14.3, D128): the TypeScript line and CSV parsers
//! (`parse-input.ts`, quirks kept) and the prompt checks.

use crate::error::{CliError, Failure};
use crate::files::{self, PROMPTS_LIMIT};
use crate::tenant::config::js_trim;
use std::ffi::OsStr;

/// The SDK content limit per prompt, in UTF-8 bytes.
pub const MAX_PROMPT_BYTES: usize = 2_097_152;

/// Why a file could not be parsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// An opening quote with no closing quote (D128).
    Unterminated,
    /// No header field is `prompt` (TS text).
    NoPromptColumn,
}

/// Whether `path` is read as CSV (`.csv` in any ASCII case; TS `/\.csv$/i`).
pub fn is_csv(path: &OsStr) -> bool {
    let bytes = path.as_encoded_bytes();
    bytes.len() >= 4 && bytes[bytes.len() - 4..].eq_ignore_ascii_case(b".csv")
}

/// The prompts of `text`: CSV when `csv`, else one per line.
pub fn parse(text: &str, csv: bool) -> Result<Vec<String>, ParseError> {
    if !csv {
        return Ok(text
            .split('\n')
            .map(js_trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect());
    }
    let rows = csv_rows(text)?;
    let Some((header, rows)) = rows.split_first() else {
        return Ok(Vec::new());
    };
    let column = header
        .iter()
        .position(|field| js_trim(field).eq_ignore_ascii_case("prompt"))
        .ok_or(ParseError::NoPromptColumn)?;
    Ok(rows
        .iter()
        .filter_map(|row| row.get(column))
        .map(|field| js_trim(field))
        .filter(|prompt| !prompt.is_empty())
        .map(str::to_owned)
        .collect())
}

/// TS `parseCsvRows`: records end at `\n`, `\r\n` or a bare `\r` outside quotes;
/// records with no field or one empty field are dropped.
fn csv_rows(text: &str) -> Result<Vec<Vec<String>>, ParseError> {
    let chars: Vec<char> = text.chars().collect();
    let mut rows = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (row, next) = csv_row(&chars, i)?;
        if !(row.is_empty() || (row.len() == 1 && row[0].is_empty())) {
            rows.push(row);
        }
        i = next;
    }
    Ok(rows)
}

/// TS `parseRow`: one record from `start`, and the index after it.
fn csv_row(chars: &[char], start: usize) -> Result<(Vec<String>, usize), ParseError> {
    let mut fields = Vec::new();
    let mut i = start;
    while i < chars.len() {
        if chars[i] == '\n' || chars[i] == '\r' {
            i += if chars[i] == '\r' && chars.get(i + 1) == Some(&'\n') {
                2
            } else {
                1
            };
            break;
        }
        if chars[i] == '"' {
            let (value, next) = quoted(chars, i)?;
            fields.push(value);
            i = next;
        } else {
            let end = chars[i..]
                .iter()
                .position(|c| matches!(c, ',' | '\n' | '\r'))
                .map_or(chars.len(), |offset| i + offset);
            fields.push(chars[i..end].iter().collect());
            i = end;
        }
        if chars.get(i) == Some(&',') {
            i += 1;
        }
    }
    Ok((fields, i))
}

/// TS `parseQuotedField`, except that a field without its closing quote is an error.
fn quoted(chars: &[char], start: usize) -> Result<(String, usize), ParseError> {
    let mut value = String::new();
    let mut i = start + 1;
    while i < chars.len() {
        if chars[i] == '"' {
            if chars.get(i + 1) == Some(&'"') {
                value.push('"');
                i += 2;
            } else {
                return Ok((value, i + 1));
            }
        } else {
            value.push(chars[i]);
            i += 1;
        }
    }
    Err(ParseError::Unterminated)
}

/// Read, parse and check the `--file` input (§14.3). Every failure is exit 1, except
/// `No prompts found in input file` (exit 2).
pub fn load(path: &OsStr) -> Result<Vec<String>, CliError> {
    let shown = files::shown(path);
    let text = files::read_text(path, PROMPTS_LIMIT)?;
    let prompts = parse(&text, is_csv(path)).map_err(|error| match error {
        ParseError::Unterminated => {
            CliError::operational(format!("{shown} has an unterminated quoted field"))
        }
        ParseError::NoPromptColumn => {
            CliError::operational("No \"prompt\" column found in CSV header")
        }
    })?;
    check(&shown, &prompts)?;
    Ok(prompts)
}

/// The prompt checks: at least one, at most [`super::MAX_PROMPTS`], each within the SDK
/// content limit (the first oversized one is reported with its 0-based index).
pub fn check(shown: &str, prompts: &[String]) -> Result<(), CliError> {
    if prompts.is_empty() {
        return Err(CliError::usage("No prompts found in input file"));
    }
    if prompts.len() > super::MAX_PROMPTS {
        return Err(CliError::operational(format!(
            "{shown} has more than {} prompts; split it into several jobs",
            super::MAX_PROMPTS
        )));
    }
    if let Some(index) = prompts
        .iter()
        .position(|prompt| prompt.len() > MAX_PROMPT_BYTES)
    {
        return Err(CliError::Operational(Failure {
            message: "AISEC_USER_REQUEST_PAYLOAD_ERROR:prompt exceeds max length of 2097152 bytes"
                .to_owned(),
            status: None,
            details: vec![format!("Prompt index {index}")],
        }));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        parse(text, false).unwrap()
    }

    fn csv(text: &str) -> Vec<String> {
        parse(text, true).unwrap()
    }

    /// The B§3 corpus: every row gives the TypeScript prompts, except the unterminated
    /// quote (refused, D128) and invalid UTF-8 (refused by the reader).
    #[test]
    fn the_typescript_corpus() {
        for (name, text, expected) in [
            ("bom.csv", "\u{feff}prompt\nhi\n", &["hi"][..]),
            ("crlf.csv", "prompt\r\na\r\nb\r\n", &["a", "b"]),
            ("cr.csv", "prompt\ra\rb", &["a", "b"]),
            (
                "multiline.csv",
                "prompt,x\n\"line1\nline2\",1\n",
                &["line1\nline2"],
            ),
            ("quoted-crlf.csv", "prompt\r\n\"a\r\nb\"\r\n", &["a\r\nb"]),
            (
                "escaped.csv",
                "prompt\n\"He said \"\"hi\"\"\"\n",
                &["He said \"hi\""],
            ),
            ("afterquote.csv", "prompt,x\n\"ab\"cd,1\n", &["ab"]),
            ("spacequote.csv", "prompt,x\n \"a,b\",1\n", &["\"a"]),
            ("shortrow.csv", "id,prompt\n1\n2,two\n", &["two"]),
            ("hdrspace.csv", "id, PROMPT \n1,  padded  \n", &["padded"]),
            ("blankrows.csv", "prompt\n\n\na\n,\n\"\"\n   \n", &["a"]),
            ("emptyfirst.csv", "\nprompt\na\n", &["a"]),
            ("first.csv", "prompt,prompt\na,b\n", &["a"]),
            ("quotedhdr.csv", "\"prompt\"\nq\n", &["q"]),
        ] {
            assert_eq!(csv(text), expected, "{name}");
        }
        for (name, text, expected) in [
            ("crlf.txt", "a\r\n\r\n b\t\r\n", &["a", "b"][..]),
            ("bom.txt", "\u{feff}first\nsecond", &["first", "second"]),
            ("nbsp.txt", "\u{a0}x\u{a0}\n y\n", &["x", "y"]),
            ("dup.txt", "same\nsame\n", &["same", "same"]),
            (
                "controls.txt",
                "a\u{7}b\u{1b}[31mred\n",
                &["a\u{7}b\u{1b}[31mred"],
            ),
        ] {
            assert_eq!(lines(text), expected, "{name}");
        }
        assert_eq!(
            parse("prompt\n\"abc\ndef\n", true),
            Err(ParseError::Unterminated)
        );
        assert_eq!(
            parse("id,text\n1,hi\n", true),
            Err(ParseError::NoPromptColumn)
        );
        for empty in ["", "prompt\n", "\n\n"] {
            assert!(csv(empty).is_empty(), "{empty:?}");
        }
        assert!(lines(" \n\t\n").is_empty());
    }

    #[test]
    fn csv_choice_ignores_ascii_case() {
        for (path, csv) in [
            ("a.csv", true),
            ("A.CSV", true),
            ("x.Csv", true),
            ("a.txt", false),
            ("csv", false),
            ("a.csv.txt", false),
            (".csv", true),
        ] {
            assert_eq!(is_csv(OsStr::new(path)), csv, "{path}");
        }
    }

    #[test]
    fn prompt_checks() {
        let message = |result: Result<(), CliError>| match result {
            Err(CliError::Operational(failure)) => (failure.message, failure.details),
            Err(CliError::Usage(message)) => (message, vec!["usage".into()]),
            other => panic!("{other:?}"),
        };
        assert_eq!(
            message(check("p.txt", &[])),
            (
                "No prompts found in input file".into(),
                vec!["usage".into()]
            )
        );
        let many = vec!["x".to_owned(); super::super::MAX_PROMPTS + 1];
        assert_eq!(
            message(check("p.txt", &many)).0,
            "p.txt has more than 10000 prompts; split it into several jobs"
        );
        assert!(check("p.txt", &many[..super::super::MAX_PROMPTS]).is_ok());
        let big = vec![
            "ok".to_owned(),
            "é".repeat(MAX_PROMPT_BYTES / 2),
            "x".repeat(MAX_PROMPT_BYTES + 1),
            "y".repeat(MAX_PROMPT_BYTES + 1),
        ];
        assert_eq!(
            message(check("p.txt", &big)),
            (
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:prompt exceeds max length of 2097152 bytes"
                    .into(),
                vec!["Prompt index 2".into()]
            )
        );
    }
}
