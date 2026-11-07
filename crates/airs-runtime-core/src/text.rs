//! Terminal-safe text and JavaScript number formatting (DESIGN §2.2, §2.7), shared by
//! the CLI renderers and the server so neither can print control characters.

use std::borrow::Cow;

/// Replace C0 control characters, DEL and C1 control characters with U+FFFD, so text
/// that came from a server or a file cannot move the cursor, retitle the window or
/// forge status lines (DESIGN §2.2). Text without them is returned unchanged.
pub fn terminal_safe(text: &str) -> Cow<'_, str> {
    let control = |c: char| c.is_control();
    if text.chars().any(control) {
        Cow::Owned(
            text.chars()
                .map(|c| if control(c) { '\u{fffd}' } else { c })
                .collect(),
        )
    } else {
        Cow::Borrowed(text)
    }
}

/// [`terminal_safe`] for multi-line server text that TypeScript prints verbatim on several
/// lines (the red team report summaries, EULA content, script output, descriptions,
/// prompts and goals; DESIGN §15.3 "Multi-line server text", D52): a line feed, a CR LF
/// pair and a tab are kept as TypeScript prints them; every other control character, a
/// lone CR included, becomes U+FFFD.
pub fn terminal_safe_lines(text: &str) -> Cow<'_, str> {
    let mut chars = text.chars().peekable();
    let mut kept = true;
    let mut out = String::with_capacity(text.len());
    while let Some(c) = chars.next() {
        let keep =
            !c.is_control() || c == '\n' || c == '\t' || (c == '\r' && chars.peek() == Some(&'\n'));
        kept &= keep;
        out.push(if keep { c } else { '\u{fffd}' });
    }
    if kept {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(out)
    }
}
/// `2^53`: integers up to this magnitude are written exactly.
const SAFE_BOUND: i128 = 9_007_199_254_740_992;

/// ECMAScript `Number::toString` of a double: `2`, `2.5`, `1e+21`,
/// `100000000000000000000`, `0.000001`, `1e-7`; `-0` is `0`.
pub fn js_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    ryu_js::Buffer::new().format(value).to_owned()
}

/// An integer as JavaScript prints it: exact within `±2^53`, otherwise the nearest
/// double through [`js_number`].
pub fn js_integer(value: i128) -> String {
    if value.abs() <= SAFE_BOUND {
        value.to_string()
    } else {
        // Out of the safe range: JavaScript holds the nearest double.
        #[allow(clippy::cast_precision_loss)]
        js_number(value as f64)
    }
}

/// A JSON number in JavaScript form.
pub fn js_json_number(number: &serde_json::Number) -> String {
    if let Some(value) = number.as_i64() {
        js_integer(i128::from(value))
    } else if let Some(value) = number.as_u64() {
        js_integer(i128::from(value))
    } else {
        js_number(number.as_f64().unwrap_or_default())
    }
}
