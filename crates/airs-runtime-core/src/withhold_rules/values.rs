//! The tests of a string (DESIGN §19.3 rules 3e, 3f, 3g and 4; choices 151, 152, 160, 161,
//! 167–170): whether it looks like a credential or a password, is a date, shows a credential,
//! holds a private key or is shaped as a JSON document, the words after an authorization
//! scheme and the credentials in `key=value` segments (choice 181), the URLs inside a text and
//! what the URL rule withholds of them, the strings of a JSON text serde_json cannot read,
//! whether a credential name's value registers whole and the credentials of a text that is never
//! printed (choice 187).

use super::names::{Leaf, registered, registrable};

/// The shortest string that can look like a credential.
pub const CREDENTIAL_LOOK: usize = 16;

/// The shortest string that can look like a password.
pub const PASSWORD_LOOK: usize = 8;

/// The separators of a word-shaped string ([`looks_like_password`]).
const WORD_SEPARATORS: [char; 5] = ['-', '_', '.', '/', ':'];

/// A string that looks like a credential: at least [`CREDENTIAL_LOOK`] characters, no
/// whitespace (choice 120) and not a date (choice 151).
pub fn looks_like_credential(text: &str) -> bool {
    text.chars().count() >= CREDENTIAL_LOOK
        && !text.chars().any(char::is_whitespace)
        && !is_date(text)
}

/// A string that looks like a password (choice 151): at least [`PASSWORD_LOOK`] characters,
/// no whitespace, and a symbol other than the word separators (`-`, `_`, `.`, `/`, `:`) or
/// a part mixing letters and digits (`hunter2x`, `p4ssw0rd`, `P@ss!word`); never an
/// absolute URL, a date or a word-shaped string, whose separated parts are each letters
/// only or digits only (`us-east-1`, `prod-eu-west-1`, `10.0.0.1`, `approle`).
pub fn looks_like_password(text: &str) -> bool {
    text.chars().count() >= PASSWORD_LOOK && password_shaped(text)
}

/// The shape of [`looks_like_password`] at any length: no whitespace, not an absolute URL nor
/// a date, and a symbol other than the word separators or a part mixing letters and digits.
/// So `abc+/==` and `hunter2` are shaped as a password and `none`, `true`, `basic`, `1234` and
/// `us-east-1` are not. (Choice 183 registered a credential key's value of this shape at any
/// length; choice 186 registers a letters-and-digits word only at [`PASSWORD_LOOK`].)
fn password_shaped(text: &str) -> bool {
    if text.chars().any(char::is_whitespace) || text.contains("://") || is_date(text) {
        return false;
    }
    text.chars()
        .any(|c| !c.is_alphanumeric() && !WORD_SEPARATORS.contains(&c))
        || text.split(WORD_SEPARATORS).any(|part| {
            part.chars().any(|c| c.is_ascii_digit()) && part.chars().any(char::is_alphabetic)
        })
}

/// Whether `text` is all ASCII digits and `len` long.
fn digits(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// Whether a text is a whole ISO 8601 date or timestamp (choice 160): `YYYY-MM-DD`, alone or
/// followed by `T`, `HH:MM`, optional `:SS`, optional fractional seconds and an optional
/// zone (`Z` or `±HH:MM`). So `2026-10-19T00:00:00.000Z` is a date and
/// `2026-01-01-k3y5ecr3t` is not.
pub fn is_date(text: &str) -> bool {
    let Some(date) = text.get(..10) else {
        return false;
    };
    let mut parts = date.split('-');
    let date_ok = matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(year), Some(month), Some(day), None)
            if digits(year, 4) && digits(month, 2) && digits(day, 2)
    );
    if !date_ok {
        return false;
    }
    match text[10..].strip_prefix('T') {
        None => text.len() == 10,
        Some(time) => is_time(time),
    }
}

/// `HH:MM`, optional `:SS`, optional `.digits`, optional `Z` or `±HH:MM`, and nothing else.
fn is_time(text: &str) -> bool {
    let (clock, zone) = match text.find(['Z', '+', '-']) {
        Some(at) => text.split_at(at),
        None => (text, ""),
    };
    let (clock, fraction) = match clock.split_once('.') {
        Some((clock, fraction)) => (clock, Some(fraction)),
        None => (clock, None),
    };
    let fields: Vec<&str> = clock.split(':').collect();
    let clock_ok = matches!(fields.len(), 2 | 3) && fields.iter().all(|field| digits(field, 2));
    let fraction_ok = fraction.is_none_or(|fraction| {
        !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit())
    });
    let zone_ok = zone.is_empty()
        || zone == "Z"
        || zone
            .strip_prefix(['+', '-'])
            .and_then(|offset| offset.split_once(':'))
            .is_some_and(|(hours, minutes)| digits(hours, 2) && digits(minutes, 2));
    clock_ok && fraction_ok && zone_ok
}

/// The authorization schemes whose next word is a credential.
pub const AUTH_SCHEMES: [&str; 3] = ["basic", "bearer", "token"];

/// The characters that join a header name or a key to the scheme written after it with no
/// space (choice 181): `Authorization:Bearer`, `Proxy-Authorization:Basic`,
/// `Authorization=Bearer`.
const SCHEME_JOINERS: [char; 2] = [':', '='];

/// The scheme a whitespace-separated word names, and the header name or key joined before
/// it (choice 181): the word's part after its last `:` or `=` ([`SCHEME_JOINERS`]), trimmed
/// of the punctuation around it (`"Bearer`, `(Basic`), when that part is an authorization
/// scheme ([`AUTH_SCHEMES`], any case). So `Bearer`, `'Authorization:Bearer` and
/// `Authorization=Token` each name a scheme, with the joined `'Authorization` for the last
/// two; `Bearer-ish` and `x:bearers` name none.
fn scheme_of(word: &str) -> Option<(&str, &str)> {
    let (joined, part) = match word.rfind(SCHEME_JOINERS) {
        Some(at) => (&word[..at], &word[at + 1..]),
        None => ("", word),
    };
    let scheme = part.trim_matches(|c: char| !c.is_alphanumeric());
    AUTH_SCHEMES
        .contains(&scheme.to_ascii_lowercase().as_str())
        .then_some((scheme, joined))
}

/// Whether a header name, trimmed of the quotes and brackets around it, names an
/// `Authorization` header (`Authorization`, `'Proxy-Authorization`, `"authorization"`).
fn names_authorization(name: &str) -> bool {
    name.trim_matches(SCHEME_WORD_TRIM)
        .to_ascii_lowercase()
        .ends_with("authorization")
}

/// The words of `text` that follow an authorization scheme ([`AUTH_SCHEMES`], any case, also
/// joined to a header name: `Authorization:Bearer`): the `<token>` of `Bearer <token>`. In a
/// credential's value each one is the credential (choice 161).
pub fn scheme_words(text: &str) -> Vec<&str> {
    let words: Vec<&str> = text.split_whitespace().collect();
    words
        .windows(2)
        .filter(|pair| scheme_of(pair[0]).is_some())
        .map(|pair| pair[1])
        .collect()
}

/// The characters trimmed from either end of the word after a scheme before it is tested
/// (choice 177): the quotes, brackets and separators around a header in a command line or a
/// prose (`'Authorization: Bearer <token>'`, `(Token <token>),`). Square brackets are kept, so
/// `[withheld]` stays itself.
const SCHEME_WORD_TRIM: &[char] = &['"', '\'', '`', '(', ')', '{', '}', '<', '>', ',', ';'];

/// Whether `word` is an HTTP Basic credential (choice 177): standard base64 of at least 8
/// characters (a multiple of 4, `=` padding only at the end) that decodes to printable ASCII
/// holding a `:` (`user:password`), so `dXNlcjpwYXNz` is one and `introduction` is not.
pub fn is_basic_credential(word: &str) -> bool {
    let bytes = word.as_bytes();
    if bytes.len() < 8 || !bytes.len().is_multiple_of(4) {
        return false;
    }
    let body = word.trim_end_matches('=');
    if bytes.len() - body.len() > 2 {
        return false;
    }
    let value = |byte: u8| -> Option<u32> {
        match byte {
            b'A'..=b'Z' => Some(u32::from(byte - b'A')),
            b'a'..=b'z' => Some(u32::from(byte - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(byte - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let mut decoded = Vec::with_capacity(body.len() * 3 / 4);
    let (mut bits, mut count) = (0_u32, 0_u32);
    for byte in body.bytes() {
        let Some(sextet) = value(byte) else {
            return false;
        };
        bits = (bits << 6) | sextet;
        count += 6;
        if count >= 8 {
            count -= 8;
            decoded.extend(u8::try_from((bits >> count) & 0xFF).ok());
        }
    }
    decoded.contains(&b':') && decoded.iter().all(|byte| (0x20..0x7F).contains(byte))
}

/// One credential a text rule finds inside a string (choices 177, 181, 183): the bytes
/// `[withheld]` replaces, and what of it joins the `SecretSet`.
///
/// It is registered only where the text says it is a credential (an `Authorization` or
/// credential header, a whole header value, a credential-named key whose value can be a
/// credential by its shape) and always [`registrable`], so a word of prose that merely
/// follows `token`, or `auth=none`, is withheld in that text and nowhere else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextCredential {
    /// The bytes the placeholder replaces: the credential, with the braces of an ODBC braced
    /// value (`PWD={…}`).
    pub span: std::ops::Range<usize>,
    /// The credential as it joins the `SecretSet` (an ODBC braced value without its braces and
    /// with each `}}` read as `}`); `None` when it is withheld in this text only.
    pub registered: Option<String>,
}

impl TextCredential {
    /// A credential whose bytes are `span` of `text`, registered when `registered` says so and
    /// it is [`registrable`].
    fn at(text: &str, span: std::ops::Range<usize>, registered: bool) -> Self {
        let value = &text[span.clone()];
        Self {
            registered: (registered && registrable(value)).then(|| value.to_owned()),
            span,
        }
    }
}

/// The credentials after an authorization scheme inside `text` (choices 177, 181, 183).
///
/// - A scheme is `Basic`, `Bearer` or `Token` ([`AUTH_SCHEMES`]) in any case, with the
///   punctuation around it ignored, also joined to a header name with `:` or `=`
///   (`Authorization:Bearer`).
/// - The word after it, trimmed of the quotes and brackets around it, is a credential when it
///   looks like a credential or a password, or (after `Basic`) is a Basic credential
///   ([`is_basic_credential`]); `[withheld]` is not one. So `Bearer sk-proj-…`,
///   `'Authorization: Bearer <token>'`, `Authorization:Bearer <token>` and
///   `Basic dXNlcjpwYXNz` each give the credential, and `a basic introduction` gives nothing.
/// - It is registered where the text says it is a credential: after `Bearer` (no prose
///   word), after an `Authorization` header name (`Authorization: Basic x`,
///   `Authorization:Token x`), or as a whole header value, the text being the capitalised
///   scheme as a header writes it and the credential (`Token x`, `Basic x`; choice 183).
/// - After the English words `token` and `basic` in any other prose (`token
///   gpt-4o-mini-2024-07-18 quota exceeded`, the two-word label `token
///   gpt-4o-mini-2024-07-18`) it is withheld in place only, so a model id there is not
///   withheld everywhere.
pub fn scheme_credential_spans(text: &str) -> Vec<TextCredential> {
    let words = words_at(text);
    let mut spans = Vec::new();
    for (index, pair) in words.windows(2).enumerate() {
        let ((_, word_before), (at, word)) = (pair[0], pair[1]);
        let Some((scheme, joined)) = scheme_of(word_before) else {
            continue;
        };
        let core = word.trim_matches(SCHEME_WORD_TRIM);
        if core.is_empty() || core == super::WITHHELD {
            continue;
        }
        let basic = scheme.eq_ignore_ascii_case("basic");
        if !(looks_like_credential(core)
            || looks_like_password(core)
            || (basic && is_basic_credential(core)))
        {
            continue;
        }
        let header_named = if joined.is_empty() {
            index > 0 && {
                let name = words[index - 1].1.trim_matches(SCHEME_WORD_TRIM);
                name.strip_suffix(SCHEME_JOINERS)
                    .is_some_and(names_authorization)
            }
        } else {
            names_authorization(joined)
        };
        let header_value =
            words.len() == 2 && joined.is_empty() && scheme.starts_with(char::is_uppercase);
        let bearer = scheme.eq_ignore_ascii_case("bearer");
        let start = at + (word.len() - word.trim_start_matches(SCHEME_WORD_TRIM).len());
        spans.push(TextCredential::at(
            text,
            start..start + core.len(),
            header_named || header_value || bearer,
        ));
    }
    spans
}

/// The credentials after an authorization scheme ([`scheme_credential_spans`]), registered
/// or not: where the text is not known to be a credential (a describing text, a generic
/// value), only these are read as one (choices 152, 161, 177).
pub fn scheme_credentials(text: &str) -> Vec<&str> {
    scheme_credential_spans(text)
        .into_iter()
        .map(|found| &text[found.span])
        .collect()
}

/// The characters that end the value of a `key=value` segment (choice 181): whitespace, the
/// separators of a connection string, a query and a cookie (`;`, `&`), and the quotes and
/// brackets that close a command line or a prose around it.
fn ends_key_value(c: char) -> bool {
    c == '&' || ends_parameters(c)
}

/// The characters that end a credential's value that is itself a run of `key=value`
/// parameters (choice 183): those of [`ends_key_value`] but `&`, which inside it joins the
/// parameters of a shared access signature (`SharedAccessSignature=sv=…&sig=…`).
fn ends_parameters(c: char) -> bool {
    c.is_whitespace() || matches!(c, ';' | '"' | '\'' | '`' | ')' | '}' | '>')
}

/// The characters that end a credential's value in a connection string's segment (choice 186):
/// the next `;` or a line break, so a password holding a space is one value.
fn ends_segment(c: char) -> bool {
    matches!(c, ';' | '\n' | '\r')
}

/// The spaces a credential name may have around its `=` (choice 186: `password = …`).
const KEY_VALUE_SPACES: [char; 2] = [' ', '\t'];

/// Whether a credential name's `key=value` value is registered (choices 183, 186): it looks like
/// a credential or a password, carries a symbol other than the word separators at any length
/// (`abc+/==`, `p@ss`), or holds whitespace inside the delimiters that make it one value (its
/// quotes, its braces, a connection string's segment). A shorter word of letters and digits
/// (`oauth2`, `sha256`, `v2.0`, `hunter2`) is not: it is withheld in that text only, so the
/// word does not withhold the same word in another row.
fn credential_value_registered(value: &str, delimited: bool) -> bool {
    looks_like_credential(value)
        || looks_like_password(value)
        || value
            .chars()
            .any(|c| !(c.is_alphanumeric() || c.is_whitespace() || WORD_SEPARATORS.contains(&c)))
        || (delimited && value.contains(char::is_whitespace))
}

/// Whether a string withheld under a credential name joins the `SecretSet` whole (rule 3g,
/// choice 187; [`super::names::Leaf::Credential`]): never a date; a value of at least
/// [`PASSWORD_LOOK`] characters once trimmed that is no placeholder or mask
/// ([`super::names::registrable`]), so a dictionary password (`{"password": "dragonfly"}`)
/// withholds its echo; and a shorter value that passes the key=value rule's test read as one
/// ([`credential_value_registered`]: it carries a symbol other than the word separators or
/// holds whitespace, `p@ss`, `a b`). So a short word (`{"password": "Bearer"}`, `hunter2`) is
/// withheld in place only, and `Bearer` elsewhere is printed; the names whose values are
/// words and that the review saw cascade (`token_type`, `api_key_name`) describe a credential
/// and are no credential names at all (choice 187.2).
pub fn registers_whole(text: &str) -> bool {
    !is_date(text)
        && (credential_value_registered(text, true)
            || (text.trim().chars().count() >= PASSWORD_LOOK && super::names::registrable(text)))
}

/// The credentials inside a text that is withheld whole and never printed (an unrevealed
/// audit request field that is not a JSON document, rule 5; choice 187), each
/// [`registrable`]: what the text rules withhold of it ([`text_withheld`]: a URL's userinfo,
/// query and fragment, a scheme credential, a credential header's value, a credential-named
/// key's value), the strings of a text shaped as a JSON document ([`unreadable_json_strings`])
/// and each whitespace-separated word that looks like a credential and is no URL and no
/// `key=value` run (whose credentials the text rules find; a base64 word's `=` padding is not
/// one). The text itself is not registered: it is never printed, and its forms in the
/// `SecretSet` would only make every later read slower.
pub fn unprinted_credentials(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    if looks_like_json_text(text) {
        out.extend(unreadable_json_strings(text));
    }
    if let Some((_, parts)) = text_withheld(text) {
        out.extend(parts);
    }
    for word in text.split_whitespace() {
        let run = word.contains('&') || word.trim_end_matches('=').contains('=');
        if looks_like_credential(word) && !run && !word.contains("://") {
            out.push(word.to_owned());
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|found| registrable(found) && seen.insert(found.clone()));
    out
}

/// An ODBC braced value after its `{` (choice 186): the bytes up to its closing `}` (a `}}` is an
/// escaped `}`), and the value with each `}}` read as `}`; `None` when it never closes.
fn braced_value(inner: &str) -> Option<(usize, String)> {
    let mut value = String::new();
    let mut chars = inner.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if c == '}' {
            if chars.peek().is_some_and(|&(_, next)| next == '}') {
                chars.next();
            } else {
                return Some((at, value));
            }
        }
        value.push(c);
    }
    None
}

/// The credentials held as `key=value` segments inside `text` (choices 181, 183, 186): an ADO,
/// ODBC or Azure connection string (`Server=x;User Id=sa;Password=…;`,
/// `AccountName=a;AccountKey=…==`), a form body, a command line (`token=sk-…`,
/// `--password=…`) or a configuration line (`password = …`).
///
/// - The key is the run of letters, digits, `_`, `-` and `.` before an `=`, from its first
///   letter, read by the name rule ([`super::names::classify`]). With a space or a tab on either
///   side of the `=` (`aws_secret_access_key = …`) only a credential name is read, so prose
///   (`a = b`, `sort key = name`) keeps its words; `==` and `=>` are not an `=` of a segment.
/// - Under a credential name (`password`, `pwd`, `account_key`, `client_secret`, `auth`,
///   `SharedAccessSignature`, `sig`) the value is withheld whatever it is, and registered as
///   [`credential_value_registered`] says: `AccountKey=abc+/==` and `Pwd=hunter2x` are
///   registered; `auth=oauth2`, `secret=sha256`, `pwd=v2.0`, a placeholder word and a mask
///   (`auth=none`, `Password=true`, `Pwd=****`) are withheld in that text only.
/// - Under a generic or token name (`token`, `key`, `access_token`) the value is withheld when
///   it looks like a credential or a password, and registered when it looks like a credential,
///   as the name rule treats a generic name's value (rule 3b); any other key is printed
///   (`AccountName=a`, `max_tokens=100`, `prompt_tokens=12`).
/// - The value runs to whitespace, `;`, `&`, a closing quote or bracket, or the text's end; a
///   quoted one (`Password="a b";`) to its closing quote. A credential's value runs further:
///   an ODBC braced one (`PWD={p@ss;w0rd};`) to its closing `}` (`}}` escapes a `}`; the braces
///   are withheld, the value without them is registered); one whose key starts a `;`-separated
///   segment (after a `;`, or first in a text that holds a `;`) to the next `;`, a line break or
///   the end, so `Password=p@ss w0rd;` is withheld whole; and one that is itself a run of
///   `key=value` parameters (`SharedAccessSignature=sv=…&sig=…` in prose) to the end of its
///   parameters (`&` does not end it).
/// - An empty value, `[withheld]` and a scheme word (`Authorization=Bearer x`, which the scheme
///   rule reads) are not one. Every registration is [`registrable`].
pub fn key_value_credential_spans(text: &str) -> Vec<TextCredential> {
    use super::names::{Class, classify};
    let key_char = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '.');
    let mut spans = Vec::new();
    let mut from = 0;
    while let Some(offset) = text[from..].find('=') {
        let mark = from + offset;
        from = mark + 1;
        let after = &text[mark + 1..];
        if after.starts_with(['=', '>']) {
            from += 1;
            continue;
        }
        let before = text[..mark].trim_end_matches(KEY_VALUE_SPACES);
        let key_start = before
            .char_indices()
            .rev()
            .take_while(|&(_, c)| key_char(c))
            .last()
            .map_or(before.len(), |(at, _)| at);
        let key = before[key_start..].trim_start_matches(|c: char| !c.is_alphabetic());
        if key.is_empty() {
            continue;
        }
        let class = classify(key);
        let credential = class >= Class::Credential;
        let value_at = mark + 1 + (after.len() - after.trim_start_matches(KEY_VALUE_SPACES).len());
        let spaced = before.len() < mark || value_at > mark + 1;
        if class < Class::Count || (spaced && !credential) {
            continue;
        }
        let after = &text[value_at..];
        // A connection string's segment: after a `;`, or first in a text that has a `;`.
        let segment = {
            let lead = text[..key_start].trim_end();
            lead.ends_with(';') || (lead.is_empty() && after.contains(';'))
        };
        // The bytes withheld, the value they hold, and whether delimiters make it one value.
        let (span, value, delimited) = match after.chars().next() {
            Some(quote @ ('"' | '\'')) => {
                let inner = &after[1..];
                let end = inner.find(quote).unwrap_or(inner.len());
                let start = value_at + 1;
                (start..start + end, inner[..end].to_owned(), true)
            }
            Some('{') if credential => match braced_value(&after[1..]) {
                Some((end, value)) => (value_at..value_at + end + 2, value, true),
                None => (value_at..text.len(), after[1..].to_owned(), true),
            },
            _ if credential && segment => {
                let end = after.find(ends_segment).unwrap_or(after.len());
                let value = after[..end].trim_end();
                (value_at..value_at + value.len(), value.to_owned(), true)
            }
            _ => {
                let mut end = after.find(ends_key_value).unwrap_or(after.len());
                if credential && after[..end].trim_end_matches('=').contains('=') {
                    end = after.find(ends_parameters).unwrap_or(after.len());
                }
                (value_at..value_at + end, after[..end].to_owned(), false)
            }
        };
        let first_word = value.split_whitespace().next().unwrap_or_default();
        if value.trim().is_empty()
            || value.starts_with(super::WITHHELD)
            || scheme_of(first_word).is_some_and(|(_, joined)| joined.is_empty())
        {
            continue;
        }
        let credential_look = looks_like_credential(&value);
        if credential || credential_look || looks_like_password(&value) {
            let registered = if credential {
                credential_value_registered(&value, delimited)
            } else {
                credential_look
            };
            from = span.end;
            spans.push(TextCredential {
                span,
                registered: (registered && registrable(&value)).then_some(value),
            });
        }
    }
    spans
}

/// Whether `name` (trimmed of the quotes and brackets around it) is a credential header as a
/// text writes one before a `:` (choice 183): shaped as a header name
/// ([`looks_like_header_name`]), written with a `-` as a header is (`X-Api-Key`, `api-key`,
/// `Ocp-Apim-Subscription-Key`, `X-Auth-Token`) or `Authorization` or `Cookie`, and a
/// credential by the name rule or by a word of [`super::names::HEADER_CREDENTIAL_WORDS`]
/// (`key`, `auth`, `token`, `secret`, …); a count header (`x-ratelimit-remaining-tokens`)
/// is not one.
fn names_credential_header(name: &str) -> bool {
    use super::names::{Class, HEADER_CREDENTIAL_WORDS, classify, normalised};
    let name = name.trim_matches(SCHEME_WORD_TRIM);
    if !looks_like_header_name(name) {
        return false;
    }
    let normal = normalised(name);
    (name.contains('-') || matches!(normal.as_str(), "authorization" | "cookie"))
        && (classify(name) >= Class::Credential
            || normal
                .split('_')
                .any(|word| HEADER_CREDENTIAL_WORDS.contains(&word)))
}

/// The credentials written as a credential header's value inside `text` (choice 183): a word
/// `<name>:` whose name is a credential header (`names_credential_header`) and the word after
/// it, or `<name>:<value>` in one word (`curl -H 'X-Api-Key: <key>'`, `-H "api-key:<key>"`,
/// `Authorization: <key>` with no scheme), the value trimmed of the quotes and brackets around
/// it, when it looks like a credential or a password. The header name says it is a
/// credential, so it is registered ([`registrable`]). A scheme word after the name
/// (`Authorization: Bearer x`, which the scheme rule reads), an absolute URL and `[withheld]`
/// are not one; prose that names a header (`the x-api-key: header`) keeps its words.
pub fn header_credential_spans(text: &str) -> Vec<TextCredential> {
    let words = words_at(text);
    let mut spans = Vec::new();
    for (index, &(at, word)) in words.iter().enumerate() {
        let lead = word.len() - word.trim_start_matches(SCHEME_WORD_TRIM).len();
        let Some(colon) = word[lead..].find(':').map(|colon| lead + colon) else {
            continue;
        };
        if !names_credential_header(&word[lead..colon]) {
            continue;
        }
        let rest = &word[colon + 1..];
        let (value_at, value) = if rest.trim_matches(SCHEME_WORD_TRIM).is_empty() {
            match words.get(index + 1) {
                Some(&next) => next,
                None => continue,
            }
        } else {
            (at + colon + 1, rest)
        };
        let core = value.trim_matches(SCHEME_WORD_TRIM);
        if core.is_empty()
            || core.starts_with(super::WITHHELD)
            || core.contains("://")
            || scheme_of(core).is_some()
            || !(looks_like_credential(core) || looks_like_password(core))
        {
            continue;
        }
        let start = value_at + (value.len() - value.trim_start_matches(SCHEME_WORD_TRIM).len());
        if spans
            .last()
            .is_some_and(|last: &TextCredential| start < last.span.end)
        {
            continue;
        }
        spans.push(TextCredential::at(text, start..start + core.len(), true));
    }
    spans
}

/// The registered credentials of the `key=value` segments inside `text`
/// ([`key_value_credential_spans`]), and of the parameters inside each withheld value (the
/// `sig` of a shared access signature), each once.
fn key_value_registrations(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for found in key_value_credential_spans(text) {
        let value = &text[found.span];
        let mut add = |part: &str| {
            if !out.iter().any(|known| known == part) {
                out.push(part.to_owned());
            }
        };
        if let Some(registered) = &found.registered {
            add(registered);
        }
        if value.len() < text.len() {
            for nested in key_value_registrations(value) {
                add(&nested);
            }
        }
    }
    out
}

/// The whitespace-separated words of `text` with the byte offset of each.
fn words_at(text: &str) -> Vec<(usize, &str)> {
    let mut words = Vec::new();
    let mut start = None;
    for (at, c) in text.char_indices() {
        match (c.is_whitespace(), start) {
            (true, Some(from)) => {
                words.push((from, &text[from..at]));
                start = None;
            }
            (false, None) => start = Some(at),
            _ => {}
        }
    }
    if let Some(from) = start {
        words.push((from, &text[from..]));
    }
    words
}

/// `text` with every one of `found` replaced by `[withheld]`, and the registered ones; `None`
/// when there is none.
fn replaced(text: &str, found: Vec<TextCredential>) -> Option<(String, Vec<String>)> {
    if found.is_empty() {
        return None;
    }
    let mut shown = String::with_capacity(text.len());
    let mut copied = 0;
    let mut parts = Vec::new();
    for TextCredential { span, registered } in found {
        shown.push_str(&text[copied..span.start]);
        shown.push_str(super::WITHHELD);
        parts.extend(registered);
        copied = span.end;
    }
    shown.push_str(&text[copied..]);
    Some((shown, parts))
}

/// The scheme rule (choices 177, 181): every credential after an authorization scheme inside
/// `text` ([`scheme_credential_spans`]), under any name, replaced by `[withheld]` in place;
/// the withheld credentials to register (the header-form ones). `None` when there is none, so
/// the rule is idempotent (`Bearer [withheld]` stays).
pub fn scheme_withheld(text: &str) -> Option<(String, Vec<String>)> {
    replaced(text, scheme_credential_spans(text))
}

/// The header rule (choice 183): every credential written as a credential header's value
/// inside `text` ([`header_credential_spans`]), under any name, replaced by `[withheld]` in
/// place; those to register. `None` when there is none (`X-Api-Key: [withheld]` stays).
pub fn header_withheld(text: &str) -> Option<(String, Vec<String>)> {
    replaced(text, header_credential_spans(text))
}

/// The key=value rule (choices 181, 183): every credential held as a `key=value` segment
/// inside `text` ([`key_value_credential_spans`]), under any name, replaced by `[withheld]` in
/// place; the ones to register, with the registered parameters inside a withheld value (the
/// `sig` of a shared access signature). `None` when there is none (`Password=[withheld]`
/// stays).
pub fn key_value_withheld(text: &str) -> Option<(String, Vec<String>)> {
    let found = key_value_credential_spans(text);
    let nested: Vec<String> = found
        .iter()
        .flat_map(|found| key_value_registrations(&text[found.span.clone()]))
        .collect();
    replaced(text, found).map(|(shown, mut parts)| {
        for part in nested {
            if !parts.contains(&part) {
                parts.push(part);
            }
        }
        (shown, parts)
    })
}

/// One text rule: the text as printed and what it registers, `None` when it withholds nothing.
type TextRule = fn(&str) -> Option<(String, Vec<String>)>;

/// The text rules of a document's string, in order (DESIGN §19.3 rule 4; choices 177, 181,
/// 183): the URL rule, the scheme rule, the header rule and the key=value rule.
const TEXT_RULES: [TextRule; 4] = [
    url_withheld,
    scheme_withheld,
    header_withheld,
    key_value_withheld,
];

/// The text rules of model output (§19.14): the URL rule and the scheme rule. The header and
/// key=value rules read names inside a text as the name rule reads members, and an answer's
/// code sample (`password=os.environ[…]`, `"api-key": os.environ[…]`) is the answer's text
/// (choices 181, 183).
const MODEL_OUTPUT_RULES: [TextRule; 2] = [url_withheld, scheme_withheld];

/// `rules` in order, each on what the one before printed: the text as printed and everything
/// they register; `None` when none withholds anything.
fn in_order(text: &str, rules: &[TextRule]) -> Option<(String, Vec<String>)> {
    let mut shown: Option<String> = None;
    let mut parts = Vec::new();
    for rule in rules {
        if let Some((next, found)) = rule(shown.as_deref().unwrap_or(text)) {
            parts.extend(found);
            shown = Some(next);
        }
    }
    shown.map(|shown| (shown, parts))
}

/// The text rules of a document's string ([`TEXT_RULES`]: the URL rule [`url_withheld`], the
/// scheme rule [`scheme_withheld`], the header rule [`header_withheld`] and the key=value rule
/// [`key_value_withheld`]), each on what the one before printed: the text as printed and
/// everything they register (each [`registrable`]). A credential-named value inside a URL's
/// query is among the URL rule's parts. `None` when no rule withholds anything.
pub fn text_withheld(text: &str) -> Option<(String, Vec<String>)> {
    in_order(text, &TEXT_RULES)
}

/// The text rules of model output ([`MODEL_OUTPUT_RULES`]: the URL rule, then the scheme
/// rule): the text as printed, nothing registered (the answer is not a service resource;
/// §19.14). `None` when neither withholds anything.
pub fn model_output_withheld(text: &str) -> Option<String> {
    in_order(text, &MODEL_OUTPUT_RULES).map(|(shown, _)| shown)
}

/// The longest `-` or `_` separated part of a [`looks_like_header_name`] string.
const HEADER_NAME_PART: usize = 20;

/// Whether a string item of a header list is a header *name* (choice 177): 1 to 64 ASCII
/// letters, digits, `-` and `_`, starting with a letter, each part between the separators
/// letters only or digits only and at most 20 long (`x-trace`, `Authorization`,
/// `x-portkey-trace-id`); never a credential-shaped word (`sk-proj-ab12…`, `hunter2x`).
/// Such an item (a `forward_headers` list of names) is printed; any other item is withheld.
pub fn looks_like_header_name(text: &str) -> bool {
    (1..=64).contains(&text.len())
        && text.starts_with(|c: char| c.is_ascii_alphabetic())
        && text.split(['-', '_']).all(|part| {
            !part.is_empty()
                && part.len() <= HEADER_NAME_PART
                && (part.bytes().all(|byte| byte.is_ascii_alphabetic())
                    || part.bytes().all(|byte| byte.is_ascii_digit()))
        })
}

/// Whether a describing string of a credential-named property's schema shows a credential
/// (rule 3e, choice 152): a word, trimmed of the punctuation around it, that looks like a
/// credential and holds a digit or a symbol other than `-` and `_` and is not an absolute
/// URL (`sk-0123456789abcdef`; not `x-portkey-api-key`), or a word after an authorization
/// scheme ([`scheme_credentials`]), or a private key ([`has_private_key`]).
pub fn shows_credential(text: &str) -> bool {
    let token_word = |word: &str| {
        let word = word.trim_matches(|c: char| !c.is_alphanumeric());
        looks_like_credential(word)
            && !word.contains("://")
            && word
                .chars()
                .any(|c| !(c.is_alphabetic() || c == '-' || c == '_'))
    };
    text.split_whitespace().any(token_word)
        || !scheme_credentials(text).is_empty()
        || has_private_key(text)
}

/// Whether a text is shaped as a JSON object or array (rule 3f, choice 167): after
/// whitespace, `{` followed (after whitespace) by `"` or `}`, or `[` followed by `{`, `[`,
/// `"`, `]` or a JSON number or literal that ends (after whitespace) at `,` or `]`. Such a
/// text that does not parse (`JSON.parse` reads a lone surrogate escape, a number beyond
/// `f64` and any depth; serde_json does not, and a truncated text reads as nothing) is
/// withheld whole; `[withheld]`, `[INFO] …`, `{{template}}` and `[1/3] …` are not shaped so.
pub fn looks_like_json_text(text: &str) -> bool {
    let text = text.trim_start();
    if let Some(rest) = text.strip_prefix('{') {
        return rest.trim_start().starts_with(['"', '}']);
    }
    let Some(rest) = text.strip_prefix('[') else {
        return false;
    };
    let rest = rest.trim_start();
    if rest.starts_with(['{', '[', '"', ']']) {
        return true;
    }
    let end = rest
        .find(|c: char| c.is_whitespace() || c == ',' || c == ']')
        .unwrap_or(rest.len());
    let (scalar, after) = rest.split_at(end);
    is_json_scalar(scalar) && after.trim_start().starts_with([',', ']'])
}

/// Whether `text` is a JSON number (of any size) or `true`, `false` or `null`.
pub fn is_json_scalar(text: &str) -> bool {
    is_json_number(text) || matches!(text, "true" | "false" | "null")
}

/// Whether `text` is a JSON number (`-?int(.digits)?([eE][+-]?digits)?`), of any size.
fn is_json_number(text: &str) -> bool {
    let digits_at = |text: &str| text.bytes().take_while(u8::is_ascii_digit).count();
    let text = text.strip_prefix('-').unwrap_or(text);
    let int = digits_at(text);
    if int == 0 || (int > 1 && text.starts_with('0')) {
        return false;
    }
    let mut rest = &text[int..];
    if let Some(fraction) = rest.strip_prefix('.') {
        let count = digits_at(fraction);
        if count == 0 {
            return false;
        }
        rest = &fraction[count..];
    }
    if let Some(exponent) = rest.strip_prefix(['e', 'E']) {
        let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        let count = digits_at(exponent);
        if count == 0 {
            return false;
        }
        rest = &exponent[count..];
    }
    rest.is_empty()
}

/// One absolute URL inside a text (rule 4, D251): byte ranges into the text. The URL runs
/// from its scheme to the next whitespace; its userinfo is everything before the last `@`
/// of the URL (choice 168: a password may hold an unencoded `/`, `?` or `#`, so an `@` in a
/// path or query withholds what precedes it, the safe failure); after the host, the path
/// ends at the first `?` or `#`, the query at the first `#`, and the fragment runs to the
/// URL's end. A part the URL does not have is `None`; one it has empty (`http://h?`) is an
/// empty range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UrlParts {
    /// The whole URL, from its scheme's first letter.
    pub span: std::ops::Range<usize>,
    /// The userinfo, without its `@`.
    pub userinfo: Option<std::ops::Range<usize>>,
    /// The query, without its `?`.
    pub query: Option<std::ops::Range<usize>>,
    /// The fragment, without its `#`.
    pub fragment: Option<std::ops::Range<usize>>,
}

/// Every absolute URL (`<scheme>://`) inside `text`, in order (rule 4, choice 169): at the
/// start of the text or after any other text (an upstream error's prose, a leading space).
/// The scheme is the run of letters, digits, `+`, `.` and `-` before `://`, from its first
/// letter (so `9ttp://` reads as `ttp://`); a `://` with no letter before it starts none.
pub fn urls_in(text: &str) -> Vec<UrlParts> {
    let bytes = text.as_bytes();
    let scheme_byte = |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'.' | b'-');
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = text[from..].find("://") {
        let mark = from + offset;
        let after = mark + 3;
        let end = text[after..]
            .find(char::is_whitespace)
            .map_or(text.len(), |at| after + at);
        let mut start = mark;
        while start > from && scheme_byte(bytes[start - 1]) {
            start -= 1;
        }
        while start < mark && !bytes[start].is_ascii_alphabetic() {
            start += 1;
        }
        if start == mark {
            from = after;
            continue;
        }
        let url = &text[after..end];
        let (userinfo, host) = match url.rfind('@') {
            Some(at) => (Some(after..after + at), after + at + 1),
            None => (None, after),
        };
        let path = host + text[host..end].find(['/', '?', '#']).unwrap_or(end - host);
        let fragment_mark = text[path..end].find('#').map(|at| path + at);
        let query_end = fragment_mark.unwrap_or(end);
        let query = text[path..query_end]
            .find('?')
            .map(|at| path + at + 1..query_end);
        let fragment = fragment_mark.map(|at| at + 1..end);
        found.push(UrlParts {
            span: start..end,
            userinfo,
            query,
            fragment,
        });
        from = end;
    }
    found
}

/// Whether `text` holds a PEM private key block (choice 170): `-----BEGIN ` followed by a
/// label that contains `PRIVATE KEY` (`PRIVATE KEY`, `RSA PRIVATE KEY`, `EC PRIVATE KEY`,
/// `ENCRYPTED PRIVATE KEY`, `OPENSSH PRIVATE KEY`, `PGP PRIVATE KEY BLOCK`) and `-----`. Such
/// a text is withheld and registered whole under any name, so a key under a neutral name
/// (`cert`, `pem`, `value`) never prints.
pub fn has_private_key(text: &str) -> bool {
    let mut rest = text;
    while let Some(at) = rest.find("-----BEGIN ") {
        let after = &rest[at + "-----BEGIN ".len()..];
        if let Some(end) = after.find("-----")
            && after[..end].contains("PRIVATE KEY")
        {
            return true;
        }
        rest = after;
    }
    false
}

/// Whether a member name may be printed inside a value that is printed for holding no text
/// (choice 165): one that shows a credential ([`shows_credential`]) may not (a map keyed by a
/// credential is withheld).
pub fn printable_member_name(name: &str) -> bool {
    !shows_credential(name)
}

/// The strings of a text shaped as a JSON document that serde_json cannot read (rule 3f,
/// choice 167), each registered as a string inside a credential ([`Leaf::Inner`]): every
/// string literal in value place, a member name only where it shows a credential
/// ([`printable_member_name`]), and every bare word but a JSON number or literal; a
/// string holding a URL registers only the parts the URL rule withholds ([`url_withheld`]).
/// Escapes are decoded; a lone surrogate escape splits its string into the parts around it.
pub fn unreadable_json_strings(text: &str) -> Vec<String> {
    let mut withheld = Vec::new();
    let withheld = &mut withheld;
    // A string holding a URL registers what the URL rule withholds of it, as the traversal
    // does, so a provider's base URL does not withhold every string that carries it.
    let leaf = |text: &str, withheld: &mut Vec<String>| {
        if urls_in(text).is_empty() {
            withheld.extend(registered(Leaf::Inner, text));
        } else if let Some((_, parts)) = url_withheld(text) {
            withheld.extend(parts);
        }
    };
    let mut chars = text.chars().peekable();
    let mut bare = String::new();
    let flush = |bare: &mut String, withheld: &mut Vec<String>| {
        if !bare.is_empty() && !is_json_scalar(bare) {
            leaf(bare, withheld);
        }
        bare.clear();
    };
    while let Some(c) = chars.next() {
        if c != '"' {
            if c.is_whitespace() || matches!(c, '{' | '}' | '[' | ']' | ',' | ':') {
                flush(&mut bare, withheld);
            } else {
                bare.push(c);
            }
            continue;
        }
        flush(&mut bare, withheld);
        let pieces = json_string_pieces(&mut chars);
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        let name = chars.peek() == Some(&':');
        for piece in pieces.iter().filter(|piece| !piece.is_empty()) {
            if !name || !printable_member_name(piece) {
                leaf(piece, withheld);
            }
        }
    }
    flush(&mut bare, withheld);
    std::mem::take(withheld)
}

/// One JSON string literal after its opening `"`, up to its closing `"` or the text's end,
/// with its escapes decoded: the pieces between lone surrogate escapes (a Rust string
/// cannot hold one); an unknown escape is kept as its character.
fn json_string_pieces(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Vec<String> {
    /// One decoded unit: a character, or the code of a `\u` escape.
    enum Unit {
        Char(char),
        Code(u32),
    }
    let mut pieces = vec![String::new()];
    // A high surrogate escape waiting for its low half.
    let mut high: Option<u32> = None;
    while let Some(c) = chars.next() {
        let unit = match c {
            '"' => break,
            '\\' => match chars.next() {
                None => break,
                Some('u') => {
                    let code = (0..4).try_fold(0, |code, _| {
                        chars
                            .next()
                            .and_then(|c| c.to_digit(16))
                            .map(|d| code * 16 + d)
                    });
                    match code {
                        Some(code) => Unit::Code(code),
                        None => continue,
                    }
                }
                Some(escaped) => Unit::Char(match escaped {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    'b' => '\u{8}',
                    'f' => '\u{c}',
                    other => other,
                }),
            },
            other => Unit::Char(other),
        };
        let decoded = match (high.take(), unit) {
            (Some(high), Unit::Code(low @ 0xDC00..=0xDFFF)) => {
                char::from_u32(0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00))
            }
            (pending, unit) => {
                if pending.is_some() {
                    pieces.push(String::new());
                }
                match unit {
                    Unit::Code(code @ 0xD800..=0xDBFF) => {
                        high = Some(code);
                        None
                    }
                    Unit::Code(0xDC00..=0xDFFF) => {
                        pieces.push(String::new());
                        None
                    }
                    Unit::Code(code) => char::from_u32(code),
                    Unit::Char(c) => Some(c),
                }
            }
        };
        if let (Some(c), Some(piece)) = (decoded, pieces.last_mut()) {
            piece.push(c);
        }
    }
    pieces
}

/// The URL rule (D251): every absolute URL inside `text` ([`urls_in`], at its start
/// or after other text, choice 169) with a userinfo, a query or a fragment, each replaced by
/// `[withheld]`; the withheld parts to register, with the credentials of the `key=value`
/// segments inside each part (a `jdbc:…?password=…` query's password, choices 181, 183), each
/// [`registrable`]. A userinfo is registered when it holds a
/// `:` (a `user:password`) or no `/`, `?` or `#`; one read up to an `@` of a path or query
/// (`https://social.example/@someone`) is withheld but not registered, so a host never
/// joins the `SecretSet` (choice 168). A part that is already `[withheld]` (an earlier pass,
/// a revealed audit field read again) is kept and is not a part (choice 144), so the rule
/// is idempotent. `None` when nothing is withheld.
pub fn url_withheld(text: &str) -> Option<(String, Vec<String>)> {
    let mut parts = Vec::new();
    let mut shown = String::with_capacity(text.len());
    let mut copied = 0;
    let mut changed = false;
    for url in urls_in(text) {
        // In the order they appear: the userinfo, the query, the fragment.
        let ranges = [
            (url.userinfo, true),
            (url.query, false),
            (url.fragment, false),
        ];
        for (range, userinfo) in ranges {
            let Some(range) = range.filter(|range| !range.is_empty()) else {
                continue;
            };
            let part = &text[range.clone()];
            shown.push_str(&text[copied..range.start]);
            shown.push_str(super::WITHHELD);
            copied = range.end;
            if part == super::WITHHELD {
                continue;
            }
            changed = true;
            if (!userinfo || part.contains(':') || !part.contains(['/', '?', '#']))
                && registrable(part)
            {
                parts.push(part.to_owned());
            }
            for nested in key_value_registrations(part) {
                if !parts.contains(&nested) {
                    parts.push(nested);
                }
            }
        }
    }
    shown.push_str(&text[copied..]);
    changed.then_some((shown, parts))
}

#[cfg(test)]
mod tests {
    use super::super::check::{schemes_withheld, urls_withheld};
    use super::*;

    #[test]
    fn private_keys_are_found_by_their_armour() {
        for text in [
            "-----BEGIN PRIVATE KEY-----\nMIIE\n-----END PRIVATE KEY-----",
            "x -----BEGIN RSA PRIVATE KEY----- y",
            "-----BEGIN OPENSSH PRIVATE KEY-----",
            "-----BEGIN ENCRYPTED PRIVATE KEY-----",
            "-----BEGIN PGP PRIVATE KEY BLOCK-----",
            "-----BEGIN CERTIFICATE-----\n-----BEGIN EC PRIVATE KEY-----",
        ] {
            assert!(has_private_key(text), "{text}");
            assert!(shows_credential(text), "{text}");
        }
        for text in [
            "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----",
            "-----BEGIN PUBLIC KEY-----",
            "BEGIN PRIVATE KEY",
            "-----BEGIN PRIVATE KEY",
        ] {
            assert!(!has_private_key(text), "{text}");
        }
    }

    /// Choice 177: a credential after a scheme is found in place under any name; prose, a
    /// short word and `[withheld]` are not one.
    #[test]
    fn scheme_credentials_are_withheld_in_place() {
        for (text, shown, part) in [
            (
                "Bearer sk-proj-abcdef0123456789",
                "Bearer [withheld]",
                "sk-proj-abcdef0123456789",
            ),
            ("Basic dXNlcjpwYXNz", "Basic [withheld]", "dXNlcjpwYXNz"),
            ("Token hunter2x", "Token [withheld]", "hunter2x"),
            (
                "curl -H 'Authorization: Bearer abcdefghij0123456789' https://h",
                "curl -H 'Authorization: Bearer [withheld]' https://h",
                "abcdefghij0123456789",
            ),
            (
                "-H \"Authorization: Basic dXNlcjpwYXNzd29yZA==\"",
                "-H \"Authorization: Basic [withheld]\"",
                "dXNlcjpwYXNzd29yZA==",
            ),
            (
                "(Token abcdefghij0123456789),",
                "(Token [withheld]),",
                "abcdefghij0123456789",
            ),
        ] {
            assert_eq!(
                scheme_withheld(text),
                Some((shown.to_owned(), vec![part.to_owned()])),
                "{text}"
            );
            assert!(!schemes_withheld(text) && schemes_withheld(shown), "{text}");
        }
        for text in [
            "a basic introduction",
            "Use a Bearer token",
            "Bearer [withheld]",
            "basic auth",
            "Token",
        ] {
            assert_eq!(scheme_withheld(text), None, "{text}");
        }
        assert!(is_basic_credential("dXNlcjpwYXNz"));
        assert!(!is_basic_credential("introduction"));
        assert!(!is_basic_credential("dXNlcjpwYXN"));
    }

    /// Choice 181: a scheme joined to its header name with no space is a scheme; a credential
    /// is registered after `Bearer`, an `Authorization` header name or as a header value, and
    /// after the prose words `token` and `basic` withheld in place only.
    #[test]
    fn scheme_registration_follows_the_header_form() {
        for (text, shown, registered) in [
            (
                "curl -H Authorization:Bearer abcdefghij0123456789 https://h",
                "curl -H Authorization:Bearer [withheld] https://h",
                true,
            ),
            (
                "-H \"Proxy-Authorization:Basic dXNlcjpwYXNz\"",
                "-H \"Proxy-Authorization:Basic [withheld]\"",
                true,
            ),
            (
                "Authorization=Token abcdefghij0123456789",
                "Authorization=Token [withheld]",
                true,
            ),
            (
                "authorization: token abcdefghij0123456789 sent",
                "authorization: token [withheld] sent",
                true,
            ),
            (
                "retry with bearer abcdefghij0123456789 later",
                "retry with bearer [withheld] later",
                true,
            ),
            (
                "token gpt-4o-mini-2024-07-18 quota exceeded",
                "token [withheld] quota exceeded",
                false,
            ),
            (
                "basic claude-3-5-sonnet-20241022 routing",
                "basic [withheld] routing",
                false,
            ),
            // Choice 183: a whole two-word text is a header value only with the capitalised
            // scheme a header writes; after the lowercase prose word it is a label.
            ("token gpt-4o-mini-2024-07-18", "token [withheld]", false),
            ("token hunter2x", "token [withheld]", false),
            ("Basic dXNlcjpwYXNzd29yZA==", "Basic [withheld]", true),
            (
                "x-api-key: Token abcdefghij0123456789 sent",
                "x-api-key: Token [withheld] sent",
                false,
            ),
        ] {
            let parts: Vec<String> = if registered {
                scheme_credentials(text)
                    .into_iter()
                    .map(str::to_owned)
                    .collect()
            } else {
                Vec::new()
            };
            assert_eq!(
                scheme_withheld(text),
                Some((shown.to_owned(), parts)),
                "{text}"
            );
            assert_eq!(scheme_credentials(text).len(), 1, "{text}");
        }
        assert_eq!(scheme_words("Authorization:Bearer hunter2x"), ["hunter2x"]);
        for text in [
            "x:bearers abcdefghij0123456789",
            "Bearer-ish abcdefghij0123456789",
        ] {
            assert_eq!(scheme_withheld(text), None, "{text}");
        }
    }

    /// Choices 181, 183, 186: a `key=value` segment under a credential name is withheld whatever
    /// its value and registered when it looks like a credential or a password or carries a
    /// symbol (a shorter letters-and-digits word is withheld in place only); under a generic or
    /// token name withheld when it looks like a credential or a password; any other key is
    /// printed.
    #[test]
    fn key_value_credentials_are_withheld_in_place() {
        for (text, shown, parts) in [
            (
                "Server=x;User Id=sa;Password=hunter2x!;",
                "Server=x;User Id=sa;Password=[withheld];",
                vec!["hunter2x!"],
            ),
            ("Uid=a;Pwd=pw;", "Uid=a;Pwd=[withheld];", vec![]),
            // Choice 183: a credential key's value is withheld whatever it is, registered only
            // when it looks like a credential or a password and is registrable.
            ("Set auth=none here", "Set auth=[withheld] here", vec![]),
            ("Password=true;", "Password=[withheld];", vec![]),
            ("Pwd=********;", "Pwd=[withheld];", vec![]),
            ("Pwd=1234;", "Pwd=[withheld];", vec![]),
            // Choice 186: a letters-and-digits word shorter than a password is withheld in
            // place only; at password length, or with a symbol, it is registered.
            ("Pwd=hunter2;", "Pwd=[withheld];", vec![]),
            ("Pwd=hunter2x;", "Pwd=[withheld];", vec!["hunter2x"]),
            ("Pwd=p@ss;", "Pwd=[withheld];", vec!["p@ss"]),
            (
                "Use auth=oauth2 for remote",
                "Use auth=[withheld] for remote",
                vec![],
            ),
            (
                "secret=sha256 and pwd=v2.0",
                "secret=[withheld] and pwd=[withheld]",
                vec![],
            ),
            (
                "SharedAccessSignature=sv=2022-11-02&sig=abcdefghij0123456789;x=y",
                "SharedAccessSignature=[withheld];x=y",
                vec![
                    "sv=2022-11-02&sig=abcdefghij0123456789",
                    "abcdefghij0123456789",
                ],
            ),
            (
                "client_secret=abc+def/ghij==&grant_type=client_credentials",
                "client_secret=[withheld]&grant_type=client_credentials",
                vec!["abc+def/ghij=="],
            ),
            (
                "AccountName=a;AccountKey=abc+/==;EndpointSuffix=x",
                "AccountName=a;AccountKey=[withheld];EndpointSuffix=x",
                vec!["abc+/=="],
            ),
            (
                "Password=\"a b;c\";Server=x",
                "Password=\"[withheld]\";Server=x",
                vec!["a b;c"],
            ),
            (
                "run --token=sk-abcdefghij0123456 now",
                "run --token=[withheld] now",
                vec!["sk-abcdefghij0123456"],
            ),
            ("key=hunter2x!", "key=[withheld]", vec![]),
        ] {
            let parts = parts.into_iter().map(str::to_owned).collect();
            assert_eq!(
                key_value_withheld(text),
                Some((shown.to_owned(), parts)),
                "{text}"
            );
            assert!(!schemes_withheld(text) && schemes_withheld(shown), "{text}");
        }
        for text in [
            "max_tokens=100 prompt_tokens=12",
            "AccountName=acct;EndpointSuffix=core.windows.net",
            "sort key=name",
            "Password=;",
            "Password=[withheld];",
            "Authorization=Bearer [withheld]",
            "auth=basic",
            "a == b",
            "=x",
        ] {
            assert_eq!(key_value_withheld(text), None, "{text}");
        }
    }

    /// Choice 186 (review: `Password=p@ss w0rd;` printed `w0rd`, and `PWD={p@ss;w0rd};` printed
    /// `w0rd}`): a credential key that starts a connection string's segment has its value read
    /// to the next `;`, a line break or the end, and an ODBC braced value to its closing `}`
    /// (`}}` escaped); the whole value is withheld and registered. Elsewhere whitespace still
    /// ends a value, and a plain key's segment is printed.
    #[test]
    fn connection_string_values_run_to_their_segment_end() {
        for (text, shown, parts) in [
            (
                "Server=db;User Id=sa;Password=p@ss w0rd;",
                "Server=db;User Id=sa;Password=[withheld];",
                vec!["p@ss w0rd"],
            ),
            (
                "Server=db; Password=my long pass phrase; Encrypt=true",
                "Server=db; Password=[withheld]; Encrypt=true",
                vec!["my long pass phrase"],
            ),
            (
                "Password=p@ss w0rd;Server=db",
                "Password=[withheld];Server=db",
                vec!["p@ss w0rd"],
            ),
            (
                "Server=db;Password=p@ss w0rd\nnext line",
                "Server=db;Password=[withheld]\nnext line",
                vec!["p@ss w0rd"],
            ),
            (
                "Driver={ODBC Driver 18};Server=db;UID=app;PWD={p@ss;w0rd};",
                "Driver={ODBC Driver 18};Server=db;UID=app;PWD=[withheld];",
                vec!["p@ss;w0rd"],
            ),
            ("PWD={a}}b;c d};x=y", "PWD=[withheld];x=y", vec!["a}b;c d"]),
            (
                "note PWD={p@ss;w0rd} end",
                "note PWD=[withheld] end",
                vec!["p@ss;w0rd"],
            ),
            ("PWD={never closed", "PWD=[withheld]", vec!["never closed"]),
            // Not a segment's start (after prose, or in a text with no `;`): whitespace ends
            // the value, as before.
            (
                "note: Password=p@ss w0rd;",
                "note: Password=[withheld] w0rd;",
                vec!["p@ss"],
            ),
            (
                "password=hunter2x! is rejected",
                "password=[withheld] is rejected",
                vec!["hunter2x!"],
            ),
        ] {
            let parts = parts.into_iter().map(str::to_owned).collect();
            assert_eq!(
                key_value_withheld(text),
                Some((shown.to_owned(), parts)),
                "{text}"
            );
            assert!(!schemes_withheld(text) && schemes_withheld(shown), "{text}");
            assert_eq!(text_withheld(shown), None, "{text}: idempotent");
        }
        for text in [
            "Driver={ODBC Driver 18};Server=db;Database=app",
            "Server=db;Password=;Uid=app",
            "Server=db;Password=[withheld];",
            "Server=db;Authorization=Bearer [withheld]",
        ] {
            assert_eq!(key_value_withheld(text), None, "{text}");
        }
    }

    /// Choice 186: spaces around the `=` are read only after a credential name (an INI or TOML
    /// line, `password = …`), so prose that writes `a = b` keeps its words; `==` and `=>` are
    /// not a segment's `=`.
    #[test]
    fn spaced_key_values_are_read_under_credential_names_only() {
        for (text, shown, parts) in [
            (
                "aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
                "aws_secret_access_key = [withheld]",
                vec!["wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"],
            ),
            (
                "set password = hunter2x! in the config",
                "set password = [withheld] in the config",
                vec!["hunter2x!"],
            ),
            (
                "[default]\naws_access_key_id = AKIAIOSFODNN7EXAMPLE\naws_secret_access_key\t=\tabc+def/ghij",
                "[default]\naws_access_key_id = AKIAIOSFODNN7EXAMPLE\naws_secret_access_key\t=\t[withheld]",
                vec!["abc+def/ghij"],
            ),
            ("then pwd= v2.0 here", "then pwd= [withheld] here", vec![]),
        ] {
            let parts = parts.into_iter().map(str::to_owned).collect();
            assert_eq!(
                key_value_withheld(text),
                Some((shown.to_owned(), parts)),
                "{text}"
            );
            assert_eq!(text_withheld(shown), None, "{text}: idempotent");
        }
        for text in [
            "if a = b then sort key = name",
            "max_tokens = 100 and token = abcdefghij0123456789xyz",
            "password == expected",
            "password => hunter2x!",
            "password =",
        ] {
            assert_eq!(key_value_withheld(text), None, "{text}");
        }
    }

    /// The model-output subset (§19.14, choices 177, 181, 183): the URL rule then the scheme
    /// rule, on what the URL rule printed, and nothing else; an answer's code sample keeps its
    /// `key=value` and header words.
    #[test]
    fn model_output_gets_the_url_and_scheme_rules_only() {
        assert_eq!(
            model_output_withheld(
                "see https://u:p4ssw0rd@h.example/x?k=v then Bearer abcdefghij0123456789; \
                 password=hunter2x! and X-Api-Key: abcdefghij0123456789"
            ),
            Some(
                "see https://[withheld]@h.example/x?[withheld] then Bearer [withheld]; \
                 password=hunter2x! and X-Api-Key: abcdefghij0123456789"
                    .to_owned()
            )
        );
        let shown =
            model_output_withheld("Bearer abcdefghij0123456789 at https://h/x?sig=abcd").unwrap();
        assert_eq!(shown, "Bearer [withheld] at https://h/x?[withheld]");
        assert_eq!(model_output_withheld(&shown), None, "idempotent");
        assert_eq!(model_output_withheld("password=os.environ['PW']"), None);
    }

    /// Choice 183: a credential header's value written in a text (`X-Api-Key: …`, `api-key:…`,
    /// `Authorization: <key>` with no scheme) is withheld in place and registered; a scheme
    /// word after the name is the scheme rule's, and a header that names no credential, a
    /// count header and prose naming a header print.
    #[test]
    fn credential_header_values_are_withheld_in_place() {
        for (text, shown, part) in [
            (
                "curl -H 'X-Api-Key: abcdefghij0123456789' https://h",
                "curl -H 'X-Api-Key: [withheld]' https://h",
                "abcdefghij0123456789",
            ),
            (
                "-H \"api-key:abcdefghij0123456789\"",
                "-H \"api-key:[withheld]\"",
                "abcdefghij0123456789",
            ),
            (
                "Ocp-Apim-Subscription-Key: 0123456789abcdef0123",
                "Ocp-Apim-Subscription-Key: [withheld]",
                "0123456789abcdef0123",
            ),
            (
                "X-Auth-Token: hunter2x!",
                "X-Auth-Token: [withheld]",
                "hunter2x!",
            ),
            (
                "Authorization: sk-proj-abcdef0123456789",
                "Authorization: [withheld]",
                "sk-proj-abcdef0123456789",
            ),
        ] {
            assert_eq!(
                header_withheld(text),
                Some((shown.to_owned(), vec![part.to_owned()])),
                "{text}"
            );
            assert!(!schemes_withheld(text) && schemes_withheld(shown), "{text}");
            assert_eq!(text_withheld(shown), None, "{text}");
        }
        for text in [
            "Authorization: Bearer abcdefghij0123456789",
            "X-Api-Key: [withheld]",
            "the x-api-key: header is required",
            "x-ratelimit-remaining-tokens: 1999",
            "x-request-id: req-abc123def456gh",
            "description: see hunter2x!",
            "https://h:8443/x",
            "x-api-key: https://vault.example/k",
        ] {
            assert_eq!(header_withheld(text), None, "{text}");
        }
    }

    /// Choice 183: the text rules in order, on what the one before printed, registering a
    /// credential-named value inside a URL's query once, from the URL rule's parts.
    #[test]
    fn text_rules_apply_in_order() {
        assert_eq!(
            text_withheld(
                "jdbc:postgresql://h/app?user=app&password=hunter2x! and Bearer abcdefghij0123456789"
            ),
            Some((
                "jdbc:postgresql://h/app?[withheld] and Bearer [withheld]".to_owned(),
                vec![
                    "user=app&password=hunter2x!".to_owned(),
                    "hunter2x!".to_owned(),
                    "abcdefghij0123456789".to_owned()
                ]
            ))
        );
        assert_eq!(text_withheld("nothing here"), None);
    }

    /// Choice 177: a header list's names, never a credential-shaped word.
    #[test]
    fn header_names_are_word_shaped() {
        for name in [
            "x-trace",
            "Authorization",
            "x-portkey-trace-id",
            "X_Env",
            "x-01",
        ] {
            assert!(looks_like_header_name(name), "{name}");
        }
        for text in [
            "sk-proj-ab12cd34",
            "hunter2x",
            "x-b3-traceid",
            "abcdefghijklmnopqrstuvwxyz",
            "Bearer x",
            "-x",
            "",
        ] {
            assert!(!looks_like_header_name(text), "{text}");
        }
    }

    #[test]
    fn urls_are_withheld_when_every_part_is() {
        assert!(urls_withheld(
            "https://[withheld]@h/p?[withheld]#[withheld]"
        ));
        assert!(urls_withheld("http://h?"));
        assert!(!urls_withheld("see https://u:p@h/"));
    }
}
