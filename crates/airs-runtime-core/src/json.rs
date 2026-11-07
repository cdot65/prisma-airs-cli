//! Order-preserving JSON values and the JSON writers.
//!
//! `serde_json` is built without `preserve_order` (enabling it would change SDK wire
//! bytes through feature unification), so its `Map` sorts keys. Tenant files need
//! their key order (doctor reports unknown keys in file order) and printed data must
//! keep the field order of the structure that produced it. This small value type
//! keeps insertion order and resolves duplicate keys like `JSON.parse` (first
//! position, last value). The writers print every number as ECMAScript
//! `Number::toString` does ([`crate::text::js_number`], DESIGN §2.2, §2.7).

use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Number;
use serde_json::ser::{CompactFormatter, Formatter, PrettyFormatter};
use std::collections::HashMap;
use std::fmt;
use std::io;

#[derive(Clone, Debug, PartialEq)]
pub enum OValue {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<OValue>),
    Object(Vec<(String, OValue)>),
}

impl OValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }
    pub fn as_object(&self) -> Option<&[(String, OValue)]> {
        match self {
            Self::Object(entries) => Some(entries),
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&[OValue]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }
    /// Look up a key in an object (`None` for other values).
    pub fn get(&self, key: &str) -> Option<&OValue> {
        self.as_object()?
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }
}

/// The number member `key` of an object as `f64` (`None` when it is absent or not a
/// number): the pagination counts and report numbers of every product.
pub fn number(value: &OValue, key: &str) -> Option<f64> {
    match value.get(key) {
        Some(OValue::Number(number)) => number.as_f64(),
        _ => None,
    }
}

/// Parse JSON text, keeping object key order.
pub fn parse(text: &str) -> serde_json::Result<OValue> {
    serde_json::from_str(text)
}

/// Whether `error` rejected text that `JSON.parse` accepts: a lone UTF-16 surrogate
/// escape (a Rust string cannot hold one), a number beyond the `f64` range (`Infinity`
/// in JavaScript) or nesting deeper than serde_json's 128 levels. Such files fail
/// closed (DESIGN §12 D44); doctor names the reason instead of "not valid JSON".
pub fn unrepresentable(error: &serde_json::Error) -> bool {
    let text = error.to_string();
    [
        "lone leading surrogate in hex escape",
        "unexpected end of hex escape",
        "number out of range",
        "recursion limit exceeded",
    ]
    .iter()
    .any(|reason| text.starts_with(reason))
}

/// Convert any serialisable value, keeping the field order it serialises with.
pub fn to_ordered<T: Serialize + ?Sized>(value: &T) -> serde_json::Result<OValue> {
    serde_json::from_str(&serde_json::to_string(value)?)
}

/// An SDK model as ordered JSON (the document every normaliser starts from); a failure
/// is `AISEC_SDK_ERROR:Output could not be serialized as JSON`.
pub fn model_value<T: Serialize>(model: &T) -> Result<OValue, crate::error::CliError> {
    to_ordered(model).map_err(|_| {
        crate::error::CliError::operational(
            "AISEC_SDK_ERROR:Output could not be serialized as JSON",
        )
    })
}

/// [`model_value`] inside a walk or a read: a serialisation failure is an SDK decode
/// failure.
pub fn page_value<T: Serialize>(model: &T) -> Result<OValue, scm::Error> {
    to_ordered(model).map_err(|_| scm::Error::new(scm::ErrorKind::Decode))
}

/// An ordered record as a `serde_json::Value` (for the planners and checks that compare
/// values; a file keeps the ordered record).
pub fn value_of(record: &OValue) -> serde_json::Value {
    serde_json::to_value(record).unwrap_or(serde_json::Value::Null)
}

/// One step into a JSON value (the reference paths of the DLP and runtime planners).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PathStep {
    Key(String),
    Index(usize),
}

/// The value at `path` within `root`, mutably.
pub fn at_path<'a>(
    root: &'a mut serde_json::Value,
    path: &[PathStep],
) -> Option<&'a mut serde_json::Value> {
    let mut node = root;
    for step in path {
        node = match step {
            PathStep::Key(key) => node.get_mut(key.as_str())?,
            PathStep::Index(index) => node.get_mut(*index)?,
        };
    }
    Some(node)
}

/// `JSON.stringify(value, null, 2)`: 2-space indent, numbers in JavaScript form.
pub fn to_pretty<T: Serialize + ?Sized>(value: &T) -> serde_json::Result<String> {
    write_with(value, JsNumbers(PrettyFormatter::with_indent(b"  ")))
}

/// `JSON.stringify(value)`: no whitespace, numbers in JavaScript form.
pub fn to_compact<T: Serialize + ?Sized>(value: &T) -> serde_json::Result<String> {
    write_with(value, JsNumbers(CompactFormatter))
}

/// The JSON text every runtime, gateway and supplychain document prints
/// (`JSON.stringify(value, null, 2)`) with every C1 control written as a `\u00XX` escape
/// (D212): a C1 character can occur only inside a JSON string, where the escape is the
/// same text.
pub fn json_text(value: &OValue) -> Result<String, crate::error::CliError> {
    let text = to_pretty(value).map_err(|_| {
        crate::error::CliError::operational(
            "AISEC_SDK_ERROR:Output could not be serialized as JSON",
        )
    })?;
    Ok(escape_c1(&text))
}

/// `\u00XX` for every U+0080–U+009F.
pub fn escape_c1(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if ('\u{80}'..='\u{9f}').contains(&c) {
            out.push_str(&format!("\\u{:04x}", u32::from(c)));
        } else {
            out.push(c);
        }
    }
    out
}

fn write_with<T: Serialize + ?Sized, F: Formatter>(
    value: &T,
    formatter: F,
) -> serde_json::Result<String> {
    let mut bytes = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, formatter);
    value.serialize(&mut serializer)?;
    // serde_json writes valid UTF-8 only.
    Ok(String::from_utf8(bytes).unwrap_or_default())
}

/// A serde_json formatter that writes numbers as ECMAScript `Number::toString`:
/// integers with `|n| <= 2^53` exactly, everything else as the nearest `f64`.
struct JsNumbers<F>(F);

impl<F: Formatter> JsNumbers<F> {
    fn number<W: ?Sized + io::Write>(writer: &mut W, text: &str) -> io::Result<()> {
        writer.write_all(text.as_bytes())
    }
}

/// Every [`Formatter`] method except the number writers forwards to the inner one.
macro_rules! forward {
    ($($name:ident($($arg:ident: $ty:ty),*);)*) => {
        $(
            fn $name<W: ?Sized + io::Write>(&mut self, writer: &mut W $(, $arg: $ty)*) -> io::Result<()> {
                self.0.$name(writer $(, $arg)*)
            }
        )*
    };
}

impl<F: Formatter> Formatter for JsNumbers<F> {
    fn write_i8<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: i8) -> io::Result<()> {
        Self::number(writer, &value.to_string())
    }
    fn write_i16<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: i16) -> io::Result<()> {
        Self::number(writer, &value.to_string())
    }
    fn write_i32<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: i32) -> io::Result<()> {
        Self::number(writer, &value.to_string())
    }
    fn write_i64<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: i64) -> io::Result<()> {
        Self::number(writer, &crate::text::js_integer(i128::from(value)))
    }
    fn write_i128<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: i128) -> io::Result<()> {
        Self::number(writer, &crate::text::js_integer(value))
    }
    fn write_u8<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: u8) -> io::Result<()> {
        Self::number(writer, &value.to_string())
    }
    fn write_u16<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: u16) -> io::Result<()> {
        Self::number(writer, &value.to_string())
    }
    fn write_u32<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: u32) -> io::Result<()> {
        Self::number(writer, &value.to_string())
    }
    fn write_u64<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: u64) -> io::Result<()> {
        Self::number(writer, &crate::text::js_integer(i128::from(value)))
    }
    fn write_u128<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: u128) -> io::Result<()> {
        let value = i128::try_from(value).unwrap_or(i128::MAX);
        Self::number(writer, &crate::text::js_integer(value))
    }
    fn write_f32<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f32) -> io::Result<()> {
        Self::number(writer, &crate::text::js_number(f64::from(value)))
    }
    fn write_f64<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        Self::number(writer, &crate::text::js_number(value))
    }
    forward! {
        write_null();
        write_bool(value: bool);
        write_number_str(value: &str);
        begin_string();
        end_string();
        write_string_fragment(fragment: &str);
        write_char_escape(char_escape: serde_json::ser::CharEscape);
        begin_array();
        end_array();
        begin_array_value(first: bool);
        end_array_value();
        begin_object();
        end_object();
        begin_object_key(first: bool);
        end_object_key();
        begin_object_value();
        end_object_value();
        write_raw_fragment(fragment: &str);
    }
}

impl Serialize for OValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::Number(value) => value.serialize(serializer),
            Self::String(value) => serializer.serialize_str(value),
            Self::Array(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Self::Object(entries) => {
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for OValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(OValueVisitor)
    }
}

struct OValueVisitor;

impl<'de> Visitor<'de> for OValueVisitor {
    type Value = OValue;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }
    fn visit_unit<E: de::Error>(self) -> Result<OValue, E> {
        Ok(OValue::Null)
    }
    fn visit_none<E: de::Error>(self) -> Result<OValue, E> {
        Ok(OValue::Null)
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<OValue, E> {
        Ok(OValue::Bool(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<OValue, E> {
        Ok(OValue::Number(value.into()))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<OValue, E> {
        Ok(OValue::Number(value.into()))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<OValue, E> {
        Number::from_f64(value)
            .map(OValue::Number)
            .ok_or_else(|| E::custom("non-finite number"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<OValue, E> {
        Ok(OValue::String(value.to_owned()))
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<OValue, E> {
        Ok(OValue::String(value))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<OValue, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(OValue::Array(items))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<OValue, A::Error> {
        let mut entries: Vec<(String, OValue)> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        while let Some((key, value)) = map.next_entry::<String, OValue>()? {
            match index.get(&key) {
                Some(&position) => entries[position].1 = value,
                None => {
                    index.insert(key.clone(), entries.len());
                    entries.push((key, value));
                }
            }
        }
        Ok(OValue::Object(entries))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn json_text_escapes_c1_only() {
        let doc = super::OValue::Object(vec![(
            "a".to_owned(),
            super::OValue::String("x\u{9b}31m\u{1b}y\u{202e}z".to_owned()),
        )]);
        assert_eq!(
            super::json_text(&doc).unwrap(),
            "{\n  \"a\": \"x\\u009b31m\\u001by\u{202e}z\"\n}"
        );
    }

    use super::*;
    use serde::Serialize;

    /// The build-graph guard (DESIGN §15.25 point 5): `serde_json` is built without
    /// `preserve_order`, so its `Map` sorts keys. If feature unification ever enabled it
    /// (a dependency or a feature of this workspace), SDK wire bytes would change and this
    /// test fails first.
    #[test]
    fn serde_json_is_built_without_preserve_order() {
        let value: serde_json::Value = serde_json::from_str(r#"{"b":1,"c":2,"a":3}"#).unwrap();
        assert_eq!(value.to_string(), r#"{"a":3,"b":1,"c":2}"#);
    }

    #[test]
    fn keeps_key_order_and_resolves_duplicates_like_json_parse() {
        let value = parse(r#"{"b":1,"a":2,"b":3}"#).unwrap();
        let keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(keys, ["b", "a"]);
        assert_eq!(value.get("b"), Some(&OValue::Number(3.into())));
    }

    #[test]
    fn numbers_are_written_in_javascript_form() {
        let value = parse(
            r#"[2.0, 1e2, -0.0, 1.5, 9007199254740992.0, 9007199254740993, 18446744073709551615,
                -9007199254740993, 3, 1e300, 1e21, 1e20, 100000000000000000000, 0.000001, 1e-7,
                123e-20, {"a":[4.0]}]"#,
        )
        .unwrap();
        assert_eq!(
            to_compact(&value).unwrap(),
            "[2,100,0,1.5,9007199254740992,9007199254740992,18446744073709552000,-9007199254740992,\
             3,1e+300,1e+21,100000000000000000000,100000000000000000000,0.000001,1e-7,1.23e-18,\
             {\"a\":[4]}]"
        );
        assert_eq!(to_pretty(&[1.0_f64, 2.5]).unwrap(), "[\n  1,\n  2.5\n]");
        assert_eq!(
            to_compact(&serde_json::json!({"b": [1, {}], "a": null})).unwrap(),
            r#"{"a":null,"b":[1,{}]}"#
        );
    }

    #[test]
    fn pretty_output_matches_json_stringify_layout() {
        #[derive(Serialize)]
        struct Row {
            zeta: f64,
            alpha: Vec<u8>,
            nested: serde_json::Value,
        }
        let row = Row {
            zeta: 5.0,
            alpha: vec![],
            nested: serde_json::json!({}),
        };
        assert_eq!(
            to_pretty(&[row]).unwrap(),
            "[\n  {\n    \"zeta\": 5,\n    \"alpha\": [],\n    \"nested\": {}\n  }\n]"
        );
        assert_eq!(to_pretty(&Vec::<u8>::new()).unwrap(), "[]");
    }

    #[test]
    fn json_parse_inputs_serde_cannot_represent_are_classified() {
        let nested = format!("{}{}", "[".repeat(200), "]".repeat(200));
        for text in [
            r#"{"x":"\ud800"}"#,
            r#"{"x":"\udc00x"}"#,
            r#"{"x":"\ud800\n"}"#,
            r#"{"x":1e400}"#,
            nested.as_str(),
        ] {
            let error = parse(text).unwrap_err();
            assert!(unrepresentable(&error), "{text:.40}: {error}");
        }
        for text in ["{", r#"{"x":}"#, r#"{"x":"\u12"}"#, "[1,]"] {
            let error = parse(text).unwrap_err();
            assert!(!unrepresentable(&error), "{text}: {error}");
        }
        assert!(
            parse(r#"{"x":"\ud83d\ude00"}"#).is_ok(),
            "a valid pair is fine"
        );
    }

    #[test]
    fn strings_escape_like_javascript() {
        let value = OValue::String("é\u{1}\"/\n".into());
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            "\"é\\u0001\\\"/\\n\""
        );
    }
}
