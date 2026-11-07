//! Rendering primitives shared by every `pretty` and `table` layout (DESIGN §2.7, §2.8):
//! the JavaScript number formatter, table cell text, display width, the English list
//! ([`join_and`]), the TS table
//! algorithm ([`table`]), the `ui.ts` pretty primitives ([`pretty`]) and chalk-compatible
//! colour ([`style`]).

pub mod pretty;
pub mod style;
pub use airs_runtime_core::text::{js_integer, js_json_number, js_number};
pub mod table;

use crate::json::OValue;
use crate::output::terminal_safe;
use unicode_width::UnicodeWidthStr;

/// The terminal display width (UAX #11, ambiguous characters narrow), used for table
/// padding (D53).
pub fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// The text of a table cell (R§2.8): absent and `null` are empty; strings are made
/// terminal-safe (D52); numbers use [`js_number`]; booleans are `true`/`false`;
/// objects and arrays are compact JSON (DEL and C1 controls replaced; JSON already
/// escapes C0 controls).
pub fn cell_text(value: Option<&OValue>) -> String {
    match value {
        None | Some(OValue::Null) => String::new(),
        Some(OValue::String(text)) => terminal_safe(text).into_owned(),
        Some(OValue::Number(number)) => js_json_number(number),
        Some(OValue::Bool(value)) => value.to_string(),
        Some(nested @ (OValue::Array(_) | OValue::Object(_))) => {
            terminal_safe(&crate::json::to_compact(nested).unwrap_or_default()).into_owned()
        }
    }
}

/// The longest prefix of at most `units` UTF-16 code units that does not split a
/// character, and whether anything was cut (D43, D54).
pub fn utf16_prefix(text: &str, units: usize) -> (&str, bool) {
    let mut used = 0;
    for (index, c) in text.char_indices() {
        used += c.len_utf16();
        if used > units {
            return (&text[..index], true);
        }
    }
    (text, false)
}

/// `a`, `a and b`, `a, b and c` (a prompt's or a reason's list of names).
pub fn join_and(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    #[test]
    fn lists_join_with_and() {
        assert_eq!(join_and(&[]), "");
        assert_eq!(join_and(&["defaults"]), "defaults");
        assert_eq!(join_and(&["a", "b"]), "a and b");
        assert_eq!(
            join_and(&["defaults", "usage limits", "rate limits"]),
            "defaults, usage limits and rate limits"
        );
    }

    #[test]
    fn numbers_follow_ecmascript_number_to_string() {
        for (value, text) in [
            (2.0, "2"),
            (2.5, "2.5"),
            (-0.0, "0"),
            (0.0, "0"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000"),
            (123_456_789_012_345_680_000.0, "123456789012345680000"),
            (0.000_001, "0.000001"),
            (1e-7, "1e-7"),
            (-1.5e-7, "-1.5e-7"),
            (0.1 + 0.2, "0.30000000000000004"),
            (f64::MAX, "1.7976931348623157e+308"),
        ] {
            assert_eq!(js_number(value), text, "{value:e}");
        }
        assert_eq!(js_integer(9_007_199_254_740_992), "9007199254740992");
        assert_eq!(js_integer(9_007_199_254_740_993), "9007199254740992");
        assert_eq!(js_integer(-9_007_199_254_740_993), "-9007199254740992");
        assert_eq!(
            js_integer(i128::from(u64::MAX)),
            "18446744073709552000",
            "beyond 2^53 the nearest double"
        );
    }

    #[test]
    fn cell_text_rules() {
        let value = parse(
            r#"{"s":"a\u001b[31m\tb","n":2.0,"f":1e21,"t":true,"z":null,"o":{"k":["\u007f",1.0]}}"#,
        )
        .unwrap();
        assert_eq!(cell_text(value.get("s")), "a\u{fffd}[31m\u{fffd}b");
        assert_eq!(cell_text(value.get("n")), "2");
        assert_eq!(cell_text(value.get("f")), "1e+21");
        assert_eq!(cell_text(value.get("t")), "true");
        assert_eq!(cell_text(value.get("z")), "");
        assert_eq!(cell_text(value.get("missing")), "");
        assert_eq!(cell_text(value.get("o")), "{\"k\":[\"\u{fffd}\",1]}");
        let nested = parse(r#"["multi\nline", "ü 日本 🚀"]"#).unwrap();
        assert_eq!(
            cell_text(Some(&nested)),
            r#"["multi\nline","ü 日本 🚀"]"#,
            "C0 stays JSON-escaped, non-ASCII is not escaped"
        );
    }

    #[test]
    fn widths_and_utf16_prefixes() {
        assert_eq!(width("prøfile"), 7);
        assert_eq!(width("日本語"), 6);
        assert_eq!(width("🚀"), 2);
        assert_eq!(width("e\u{301}"), 1);
        assert_eq!(width("\u{fffd}"), 1);
        let long = format!("{}🚀tail", "d".repeat(79));
        assert_eq!(utf16_prefix(&long, 80), (&long[..79], true));
        assert_eq!(utf16_prefix("short", 80), ("short", false));
        let exact = "x".repeat(80);
        assert_eq!(utf16_prefix(&exact, 80), (exact.as_str(), false));
    }
}
