//! The bulk-scan job state (DESIGN §14.9, D118): the Rust-only versioned schema, its
//! writer (the §2.2 two-space layout plus `"\n"`, keys in schema order), the strict
//! reader and the invariants checked on load and before every write.

use super::{
    AttemptOutcome, ItemStatus, MAX_BATCH_SIZE, MAX_PROMPTS, is_job_id, is_scan_id, is_stamp,
};
use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// `format`.
pub const FORMAT: &str = "airs-cli-rs.bulk-scan";
/// `version`.
pub const VERSION: u64 = 1;
pub const NAMED_VERSION: u64 = 2;

/// A result's `action`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Allow,
    Block,
    Failed,
}

impl Action {
    pub fn name(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Block => "block",
            Self::Failed => "failed",
        }
    }
}

/// Detector flags in the order the row reported them (only the keys it reported).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Detections(pub Vec<(String, bool)>);

impl Detections {
    pub fn get(&self, key: &str) -> Option<bool> {
        self.0
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| *value)
    }

    /// Set `key` (an existing key keeps its position).
    pub fn set(&mut self, key: &str, value: bool) {
        match self.0.iter_mut().find(|(name, _)| name == key) {
            Some((_, existing)) => *existing = value,
            None => self.0.push((key.to_owned(), value)),
        }
    }
}

impl Serialize for Detections {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Detections {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Flags;
        impl<'de> Visitor<'de> for Flags {
            type Value = Detections;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an object of booleans")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Detections, A::Error> {
                let mut flags = Detections::default();
                while let Some((key, value)) = map.next_entry::<String, bool>()? {
                    if flags.get(&key).is_some() {
                        return Err(serde::de::Error::custom("duplicate detection key"));
                    }
                    flags.0.push((key, value));
                }
                Ok(flags)
            }
        }
        deserializer.deserialize_map(Flags)
    }
}

/// A normalised result (§14.7), with the §8.3 withholding already applied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemResult {
    pub action: Action,
    pub category: String,
    pub triggered: bool,
    pub detections: Detections,
    pub scan_id: String,
    pub report_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One input prompt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Item {
    pub index: u64,
    pub req_id: u64,
    pub prompt: String,
    pub status: ItemStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ItemResult>,
}

/// One POST attempt (the journal).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Attempt {
    pub id: u64,
    pub indices: Vec<u64>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub outcome: AttemptOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_id: Option<String>,
}

/// A rewrite of the output in progress (§14.12).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NextOutput {
    pub dev: String,
    pub ino: String,
    pub temp: String,
}

/// The job's output file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputRecord {
    pub path: String,
    pub dev: String,
    pub ino: String,
    pub next: Option<NextOutput>,
}

/// The whole state file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobState {
    pub format: String,
    pub version: u64,
    pub job_id: String,
    pub tenant: String,
    pub scan_endpoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scanner_reference: Option<crate::tenant::scanners::ScannerReference>,
    pub profile: String,
    pub session_id: String,
    pub batch_size: u64,
    pub output: OutputRecord,
    pub created_at: String,
    pub updated_at: String,
    pub sequence: u64,
    pub attempts: Vec<Attempt>,
    pub items: Vec<Item>,
}

/// What a new job starts from.
pub struct NewJob<'a> {
    pub job_id: &'a str,
    pub tenant: &'a str,
    pub scan_endpoint: &'a str,
    pub profile: &'a str,
    pub session_id: &'a str,
    pub batch_size: u64,
    pub output_path: &'a str,
    pub output: super::Identity,
    pub prompts: Vec<String>,
}

impl JobState {
    /// Every item `pending`, no attempt, `sequence` 0 (the first checkpoint makes it 1).
    pub fn new(job: NewJob<'_>) -> Self {
        let now = super::now_stamp();
        Self {
            format: FORMAT.to_owned(),
            version: VERSION,
            job_id: job.job_id.to_owned(),
            tenant: job.tenant.to_owned(),
            scan_endpoint: job.scan_endpoint.to_owned(),
            scanner_reference: None,
            profile: job.profile.to_owned(),
            session_id: job.session_id.to_owned(),
            batch_size: job.batch_size,
            output: OutputRecord {
                path: job.output_path.to_owned(),
                dev: job.output.dev.to_string(),
                ino: job.output.ino.to_string(),
                next: None,
            },
            created_at: now.clone(),
            updated_at: now,
            sequence: 0,
            attempts: Vec::new(),
            items: job
                .prompts
                .into_iter()
                .enumerate()
                .map(|(index, prompt)| Item {
                    index: index as u64,
                    req_id: index as u64,
                    prompt,
                    status: ItemStatus::Pending,
                    attempt: None,
                    scan_id: None,
                    receipt_report_id: None,
                    result: None,
                })
                .collect(),
        }
    }

    /// The serialised bytes (two-space layout plus `"\n"`).
    pub fn to_bytes(&self) -> Option<Vec<u8>> {
        let mut text = crate::json::to_pretty(self).ok()?;
        text.push('\n');
        Some(text.into_bytes())
    }

    /// Items counted as submitted for the interruption context (`submitted`,
    /// `complete` or `failed`).
    pub fn submitted_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| {
                matches!(
                    item.status,
                    ItemStatus::Submitted | ItemStatus::Complete | ItemStatus::Failed
                )
            })
            .count()
    }

    /// The attempt with `id`.
    pub fn attempt_mut(&mut self, id: u64) -> Option<&mut Attempt> {
        self.attempts.iter_mut().find(|attempt| attempt.id == id)
    }

    /// The invariants (§14.9): `Err` carries the fixed reason.
    pub fn validate(&self) -> Result<(), String> {
        if self.format != FORMAT || ![VERSION, NAMED_VERSION].contains(&self.version) {
            return Err("unsupported format or version".into());
        }
        match (&self.scanner_reference, self.version) {
            (None, VERSION) => {}
            (Some(reference), NAMED_VERSION)
                if reference.valid()
                    && crate::tenant::scanners::canonical_endpoint(&self.scan_endpoint)
                        .as_deref()
                        == Some(reference.endpoint.as_str()) => {}
            _ => return Err("scanner reference does not match state version or endpoint".into()),
        }
        if !is_job_id(&self.job_id) {
            return Err("jobId has the wrong shape".into());
        }
        if !crate::store::valid_tenant_name(&self.tenant) {
            return Err("tenant is not a valid tenant name".into());
        }
        if self.scan_endpoint.is_empty() {
            return Err("scanEndpoint is empty".into());
        }
        let profile_units = self.profile.encode_utf16().count();
        if !(1..=100).contains(&profile_units) {
            return Err("profile must be 1-100 characters".into());
        }
        if self.session_id.encode_utf16().count() > 100 {
            return Err("sessionId must be at most 100 characters".into());
        }
        if !(1..=MAX_BATCH_SIZE).contains(&self.batch_size) {
            return Err("batchSize must be a positive safe integer".into());
        }
        if self.output.path.is_empty()
            || !std::path::Path::new(&self.output.path).is_absolute()
            || !decimal(&self.output.dev)
            || !decimal(&self.output.ino)
            || self.output.next.as_ref().is_some_and(|next| {
                !decimal(&next.dev) || !decimal(&next.ino) || next.temp.is_empty()
            })
        {
            return Err("output is not a valid output record".into());
        }
        if !is_stamp(&self.created_at) || !is_stamp(&self.updated_at) {
            return Err("timestamps must be UTC ISO date-times".into());
        }
        if self.items.is_empty() || self.items.len() > MAX_PROMPTS {
            return Err("items must hold 1 to 10000 prompts".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for (position, item) in self.items.iter().enumerate() {
            if !seen.insert(item.index) {
                return Err(format!("duplicate input index {}", item.index));
            }
            if item.index != position as u64 {
                return Err("prompt entries must remain in input order".into());
            }
            if item.req_id != item.index {
                return Err("reqId must match the stable input index".into());
            }
            let status = item.status.name();
            let with_scan = matches!(
                item.status,
                ItemStatus::Submitted | ItemStatus::Complete | ItemStatus::Failed
            );
            match (&item.scan_id, with_scan) {
                (Some(id), true) if is_scan_id(id) => {}
                (_, true) => return Err(format!("{status} entries require a scanId")),
                (Some(_), false) => {
                    return Err(format!("{status} entries cannot contain a scanId"));
                }
                (None, false) => {}
            }
            if !with_scan && item.receipt_report_id.is_some() {
                return Err(format!("{status} entries cannot contain a receipt"));
            }
            let terminal = matches!(item.status, ItemStatus::Complete | ItemStatus::Failed);
            match (&item.result, terminal) {
                (None, true) => return Err(format!("{status} entries require a result")),
                (Some(_), false) => {
                    return Err(format!("{status} entries cannot contain a result"));
                }
                _ => {}
            }
            if let Some(result) = &item.result {
                if Some(&result.scan_id) != item.scan_id.as_ref() {
                    return Err("stored result scanId does not match its receipt".into());
                }
                if item.status == ItemStatus::Failed && result.action != Action::Failed {
                    return Err("failed entries require a failed result".into());
                }
                if item.status == ItemStatus::Complete && result.action == Action::Failed {
                    return Err("complete entries cannot contain a failed result".into());
                }
            }
            match (item.status, item.attempt) {
                (ItemStatus::Submitting, None) => {
                    return Err("submitting entries require an attempt".into());
                }
                (_, Some(id)) => {
                    let Some(attempt) = self.attempts.iter().find(|attempt| attempt.id == id)
                    else {
                        return Err(format!("entry {} names a missing attempt", item.index));
                    };
                    let in_flight = attempt.outcome == AttemptOutcome::InFlight;
                    if in_flight != (item.status == ItemStatus::Submitting) {
                        return Err(
                            "submitting entries must belong to the attempt in flight".into()
                        );
                    }
                }
                (_, None) => {}
            }
        }
        let mut in_flight = 0;
        for (position, attempt) in self.attempts.iter().enumerate() {
            if attempt.id != position as u64 + 1 {
                return Err("attempt ids must count from 1".into());
            }
            if attempt.indices.is_empty()
                || attempt.indices.len() > super::CHUNK_SIZE
                || attempt
                    .indices
                    .iter()
                    .any(|index| *index >= self.items.len() as u64)
            {
                return Err("every attempt index must name an item".into());
            }
            if !is_stamp(&attempt.started_at)
                || attempt
                    .ended_at
                    .as_deref()
                    .is_some_and(|stamp| !is_stamp(stamp))
            {
                return Err("timestamps must be UTC ISO date-times".into());
            }
            let flying = attempt.outcome == AttemptOutcome::InFlight;
            in_flight += usize::from(flying);
            if flying == attempt.ended_at.is_some() {
                return Err("only the attempt in flight has no end".into());
            }
            if (attempt.outcome == AttemptOutcome::Accepted) != attempt.scan_id.is_some() {
                return Err("accepted attempts require a scanId".into());
            }
        }
        if in_flight > 1 {
            return Err("at most one attempt may be in flight".into());
        }
        let mut receipts: std::collections::BTreeMap<&str, (usize, Option<&str>)> =
            std::collections::BTreeMap::new();
        for item in &self.items {
            if let Some(scan_id) = &item.scan_id {
                let entry = receipts
                    .entry(scan_id)
                    .or_insert((0, item.receipt_report_id.as_deref()));
                entry.0 += 1;
                if entry.0 > super::CHUNK_SIZE || entry.1 != item.receipt_report_id.as_deref() {
                    return Err("entries of one scanId must share one receipt".into());
                }
            }
        }
        Ok(())
    }
}

fn decimal(text: &str) -> bool {
    !text.is_empty() && text.len() <= 20 && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// Why a state file was refused (§14.9 "Loading").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// Not JSON, or JSON the parser refuses (the D44 classes).
    Malformed,
    /// A TypeScript v2 or legacy state (no `format`, a `version` or `scanIds` key).
    TypeScript,
    /// `format` is ours and `version` is newer.
    Newer,
    /// Anything else: the fixed reason.
    Invalid(String),
}

/// Parse and validate state bytes (`resume-poll`, §14.13; the state-spec round trip).
pub fn parse(bytes: &[u8]) -> Result<JobState, LoadError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| LoadError::Malformed)?;
    if let serde_json::Value::Object(object) = &value {
        match object.get("format") {
            None if object.contains_key("version") || object.contains_key("scanIds") => {
                return Err(LoadError::TypeScript);
            }
            Some(serde_json::Value::String(format))
                if format == FORMAT
                    && object
                        .get("version")
                        .and_then(serde_json::Value::as_u64)
                        .is_some_and(|version| version > NAMED_VERSION) =>
            {
                return Err(LoadError::Newer);
            }
            _ => {}
        }
    }
    let state: JobState = serde_json::from_value(value).map_err(|_| {
        LoadError::Invalid("the file does not match the bulk-scan state schema".into())
    })?;
    state.validate().map_err(LoadError::Invalid)?;
    Ok(state)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub(crate) const SCAN_A: &str = "00000000-0000-4000-8000-000000007001";

    pub(crate) fn sample() -> JobState {
        JobState::new(NewJob {
            job_id: "20261017T120000Z-3f9c0a1b2c3d4e5f",
            tenant: "prod",
            scan_endpoint: "http://127.0.0.1:1/scan",
            profile: "Prisma AIRS Harness",
            session_id: "prisma-airs-cli-bulk-mgx1a2b3",
            batch_size: 25,
            output_path: "/abs/out.csv",
            output: super::super::Identity {
                dev: 16_777_230,
                ino: 1_234_567,
            },
            prompts: vec!["first".into(), "second \"two\"".into(), "third".into()],
        })
    }

    fn complete(state: &mut JobState) {
        state.attempts.push(Attempt {
            id: 1,
            indices: vec![0, 1],
            started_at: super::super::now_stamp(),
            ended_at: Some(super::super::now_stamp()),
            outcome: AttemptOutcome::Accepted,
            http_status: Some(200),
            error: None,
            scan_id: Some(SCAN_A.into()),
        });
        for index in 0..2 {
            let item = &mut state.items[index];
            item.status = ItemStatus::Complete;
            item.attempt = Some(1);
            item.scan_id = Some(SCAN_A.into());
            item.receipt_report_id = Some(format!("R{SCAN_A}"));
            item.result = Some(ItemResult {
                action: Action::Allow,
                category: "benign".into(),
                triggered: false,
                detections: Detections(vec![("injection".into(), false)]),
                scan_id: SCAN_A.into(),
                report_id: format!("R{SCAN_A}"),
                error: None,
            });
        }
    }

    /// state-spec #1: the serialiser's bytes carry the format, version, profile, each
    /// item's correlation fields and `updatedAt`.
    #[test]
    fn a_save_carries_the_correlation_fields() {
        let mut state = sample();
        complete(&mut state);
        let text = String::from_utf8(state.to_bytes().unwrap()).unwrap();
        assert!(text.starts_with("{\n  \"format\": \"airs-cli-rs.bulk-scan\",\n  \"version\": 1,\n  \"jobId\": \"20261017T120000Z-3f9c0a1b2c3d4e5f\",\n"), "{text}");
        assert!(text.ends_with("}\n"));
        for needle in [
            "\"profile\": \"Prisma AIRS Harness\"",
            "\"updatedAt\": ",
            "\"index\": 1",
            "\"reqId\": 1",
            "\"prompt\": \"second \\\"two\\\"\"",
            &format!("\"scanId\": \"{SCAN_A}\""),
            "\"next\": null",
            "\"endedAt\": ",
        ] {
            assert!(text.contains(needle), "{needle}: {text}");
        }
        let keys: Vec<&str> = text
            .lines()
            .filter(|line| line.starts_with("  \"") && !line.starts_with("   "))
            .map(|line| line.trim().split('"').nth(1).unwrap())
            .collect();
        assert_eq!(
            keys,
            [
                "format",
                "version",
                "jobId",
                "tenant",
                "scanEndpoint",
                "profile",
                "sessionId",
                "batchSize",
                "output",
                "createdAt",
                "updatedAt",
                "sequence",
                "attempts",
                "items"
            ]
        );
    }

    /// state-spec #6: a load round-trips.
    #[test]
    fn a_load_round_trips() {
        let mut state = sample();
        complete(&mut state);
        let parsed = parse(&state.to_bytes().unwrap()).unwrap();
        assert_eq!(parsed, state);
    }

    /// state-spec #4 (the invariant half) and #6: invalid states are refused with fixed
    /// reasons.
    #[test]
    fn invalid_states_are_refused() {
        let reason = |edit: &dyn Fn(&mut JobState)| {
            let mut state = sample();
            complete(&mut state);
            edit(&mut state);
            let validated = state.validate().unwrap_err();
            let bytes = state.to_bytes().unwrap();
            assert_eq!(parse(&bytes), Err(LoadError::Invalid(validated.clone())));
            validated
        };
        assert_eq!(
            reason(&|s| s.batch_size = 0),
            "batchSize must be a positive safe integer"
        );
        assert_eq!(reason(&|s| s.items[1].index = 0), "duplicate input index 0");
        assert_eq!(
            reason(&|s| s.items.swap(1, 2)),
            "prompt entries must remain in input order"
        );
        assert_eq!(
            reason(&|s| s.items[2].req_id = 7),
            "reqId must match the stable input index"
        );
        assert_eq!(
            reason(&|s| {
                s.items[0].scan_id = None;
                s.items[0].receipt_report_id = None;
            }),
            "complete entries require a scanId"
        );
        assert_eq!(
            reason(&|s| s.items[0].result.as_mut().unwrap().scan_id =
                "00000000-0000-4000-8000-000000000999".into()),
            "stored result scanId does not match its receipt"
        );
        assert_eq!(
            reason(&|s| s.items[2].scan_id = Some(SCAN_A.into())),
            "pending entries cannot contain a scanId"
        );
        assert_eq!(
            reason(&|s| s.items[0].result.as_mut().unwrap().action = Action::Failed),
            "complete entries cannot contain a failed result"
        );
        assert_eq!(
            reason(&|s| s.items[2].status = ItemStatus::Submitting),
            "submitting entries require an attempt"
        );
        assert_eq!(
            reason(&|s| s.profile.clear()),
            "profile must be 1-100 characters"
        );
        assert_eq!(
            reason(&|s| s.items[0].receipt_report_id = Some("other".into())),
            "entries of one scanId must share one receipt"
        );
        assert_eq!(
            reason(&|s| s.attempts[0].ended_at = None),
            "only the attempt in flight has no end"
        );
    }

    #[test]
    fn named_state_requires_a_valid_reference_and_legacy_state_forbids_one() {
        let mut state = sample();
        state.version = NAMED_VERSION;
        assert!(state.validate().is_err());
        state.scanner_reference = Some(crate::tenant::scanners::ScannerReference {
            name: "monitor".into(),
            revision: "00000000-0000-4000-8000-000000000001".into(),
            application_id: "app-a".into(),
            endpoint: crate::tenant::scanners::canonical_endpoint(&state.scan_endpoint).unwrap(),
        });
        assert_eq!(state.validate(), Ok(()));
        assert_eq!(parse(&state.to_bytes().unwrap()), Ok(state.clone()));
        state.version = VERSION;
        assert!(state.validate().is_err());
        state.version = NAMED_VERSION;
        state.scanner_reference.as_mut().unwrap().endpoint = "https://different.example".into();
        assert!(state.validate().is_err());
    }

    /// state-spec #6: malformed, TypeScript (v2 and legacy) and newer states.
    #[test]
    fn foreign_states_are_classified() {
        assert_eq!(parse(b"{"), Err(LoadError::Malformed));
        assert_eq!(parse(b"{\"a\": \"\\ud800\"}"), Err(LoadError::Malformed));
        assert_eq!(
            parse(br#"{"version":2,"profile":"p","outputFile":"/o","batchSize":25,"items":[]}"#),
            Err(LoadError::TypeScript)
        );
        assert_eq!(
            parse(br#"{"scanIds":["a"],"profile":"p","promptCount":1}"#),
            Err(LoadError::TypeScript)
        );
        assert_eq!(
            parse(br#"{"format":"airs-cli-rs.bulk-scan","version":3}"#),
            Err(LoadError::Newer)
        );
        assert!(matches!(parse(b"[]"), Err(LoadError::Invalid(_))));
        let mut state = sample();
        let mut value: serde_json::Value =
            serde_json::from_slice(&state.to_bytes().unwrap()).unwrap();
        value["extra"] = serde_json::json!(1);
        assert!(matches!(
            parse(value.to_string().as_bytes()),
            Err(LoadError::Invalid(_))
        ));
        state.items[0].prompt = "x".into();
        assert!(parse(&state.to_bytes().unwrap()).is_ok());
    }
}
