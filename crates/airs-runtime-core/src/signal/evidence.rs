use super::invalid;
use crate::error::CliError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub start_ms: i64,
    pub end_ms: i64,
    pub interval: u64,
    pub unit: String,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Complete,
    Partial,
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub name: String,
    pub status: Status,
    pub required: bool,
    pub records: u64,
    pub pages: u64,
    pub unit: String,
    pub population: String,
    pub started_ms: i64,
    pub ended_ms: i64,
    pub notes: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub app_id: String,
    pub app_name: String,
    pub session_id: String,
    pub scan_id: String,
    pub sub_request: Option<u64>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SnippetState {
    Absent,
    Empty,
    Present,
    Withheld,
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Snippet {
    pub family: String,
    pub state: SnippetState,
    pub excerpts: Vec<String>,
    pub variants: u64,
    pub truncated: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Detection {
    pub detector: String,
    pub direction: String,
    pub verdict: String,
    pub action: String,
    pub report_id: String,
    pub row: u64,
    pub snippets: Vec<Snippet>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub identity: Identity,
    pub timestamp_ms: Option<i64>,
    pub profile: Option<String>,
    pub verdict: String,
    pub action: String,
    pub report_ids: Vec<String>,
    pub detections: Vec<Detection>,
    pub detector_hints: Vec<String>,
    pub sources: Vec<String>,
    pub notes: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Protection {
    pub detector: String,
    pub direction: String,
    pub action: String,
    pub active: bool,
    pub location: String,
    pub confidence: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    pub revision: Option<u64>,
    pub protections: Vec<Protection>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Application {
    pub id: String,
    pub name: String,
    pub profile: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Sampling {
    pub algorithm: String,
    pub candidates: u64,
    pub selected_report_ids: Vec<String>,
    pub uncovered_groups: Vec<String>,
    pub limit: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Budgets {
    pub seconds: u64,
    pub concurrency: u64,
    pub requests: u64,
    pub max_pages: u64,
    pub max_events: u64,
    pub max_reports: u64,
    pub attempts: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    pub name: String,
    pub value: Option<f64>,
    pub unit: String,
    pub scale: Option<String>,
    pub population: String,
    pub window: String,
    pub source: String,
    pub status: Status,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub schema_version: u32,
    pub rules_version: u32,
    pub redaction_version: u32,
    pub collection_id: String,
    pub digest: String,
    pub cli_version: String,
    pub sdk_version: String,
    pub tenant: String,
    pub app_ids: Vec<String>,
    pub window: Window,
    pub collected_ms: i64,
    pub sources: Vec<Source>,
    pub events: Vec<Event>,
    pub profiles: Vec<Profile>,
    pub applications: Vec<Application>,
    pub metrics: Vec<Metric>,
    pub sampling: Sampling,
    pub budgets: Budgets,
}
impl Evidence {
    pub fn digest(&self) -> Result<String, CliError> {
        let mut value = serde_json::to_value(self).map_err(|_| invalid())?;
        value.as_object_mut().ok_or_else(invalid)?.remove("digest");
        // serde_json without preserve_order uses sorted maps; only integer counts/times.
        let bytes = serde_json::to_vec(&value).map_err(|_| invalid())?;
        Ok(format!("{:x}", Sha256::digest(bytes)))
    }
    pub fn seal(&mut self) -> Result<(), CliError> {
        self.digest = self.digest()?;
        self.validate()
    }
    pub fn parse(text: &str) -> Result<Self, CliError> {
        let strict: Strict = serde_json::from_str(text).map_err(|_| invalid())?;
        let evidence: Self = serde_json::from_value(strict.0).map_err(|_| invalid())?;
        evidence.validate()?;
        Ok(evidence)
    }
    pub fn validate(&self) -> Result<(), CliError> {
        self.validate_inner(true)
    }
    /// Internal normalized collection can preserve conflicting report ownership.
    /// Published version1 evidence still requires its strict, unambiguous contract.
    pub fn seal_native(&mut self) -> Result<(), CliError> {
        self.digest = self.digest()?;
        self.validate_inner(false)
    }
    pub fn validate_native(&self) -> Result<(), CliError> {
        self.validate_inner(false)
    }
    fn validate_inner(&self, unique_reports: bool) -> Result<(), CliError> {
        let b = &self.budgets;
        if self.schema_version != 1
            || !matches!(self.rules_version, 1 | 2)
            || self.redaction_version != 1
            || self.sdk_version != "0.2.4"
            || uuid::Uuid::parse_str(&self.collection_id).is_err()
            || self.window.start_ms >= self.window.end_ms
            || self.window.interval == 0
            || !["hour", "hours", "day", "days"].contains(&self.window.unit.as_str())
            || self.events.len() > 10000
            || self.sources.len() > 64
            || self.metrics.len() > 10000
            || self.profiles.len() > 10000
            || self.applications.len() > 10000
            || self.app_ids.len() > 1000
            || !(1..=3600).contains(&b.seconds)
            || !(1..=16).contains(&b.concurrency)
            || !(1..=10000).contains(&b.requests)
            || !(1..=100).contains(&b.max_pages)
            || !(1..=10000).contains(&b.max_events)
            || b.max_reports > if self.rules_version >= 2 { 10 } else { 1000 }
            || b.attempts > b.requests
            || self.events.len() as u64 > b.max_events
            || self.sampling.selected_report_ids.len() as u64 > self.sampling.limit
            || self.sampling.limit != b.max_reports
            || self.digest != self.digest()?
        {
            return Err(invalid());
        }
        let multiplier = if self.window.unit.starts_with("hour") {
            3_600_000
        } else {
            86_400_000
        };
        let duration = self
            .window
            .interval
            .checked_mul(multiplier)
            .and_then(|v| i64::try_from(v).ok());
        if duration != self.window.end_ms.checked_sub(self.window.start_ms)
            || self.collected_ms < self.window.end_ms
        {
            return Err(invalid());
        }
        let tree = serde_json::to_value(self).map_err(|_| invalid())?;
        bounded(&tree, 0)?;
        let mut sources = BTreeSet::new();
        for source in &self.sources {
            if !sources.insert(&source.name) || source.started_ms > source.ended_ms {
                return Err(invalid());
            }
        }
        if self.metrics.iter().any(|m| {
            !sources.contains(&m.source)
                || m.value.is_some_and(|v| {
                    !v.is_finite() || v < 0.0 || v > crate::limits::MAX_SAFE_INTEGER as f64
                })
        }) {
            return Err(invalid());
        }
        let mut profiles = BTreeSet::new();
        if self
            .profiles
            .iter()
            .any(|p| !profiles.insert((&p.name, p.revision)))
        {
            return Err(invalid());
        }
        let mut apps = BTreeSet::new();
        if self
            .applications
            .iter()
            .any(|a| !apps.insert((&a.id, &a.name)))
        {
            return Err(invalid());
        }
        let mut selected = BTreeSet::new();
        let report_ids: BTreeSet<_> = self
            .events
            .iter()
            .flat_map(|e| e.report_ids.iter())
            .collect();
        if self
            .sampling
            .selected_report_ids
            .iter()
            .any(|id| !selected.insert(id) || !report_ids.contains(id))
        {
            return Err(invalid());
        }
        let mut report_owners = std::collections::BTreeMap::new();
        let mut ids = BTreeSet::new();
        for event in &self.events {
            if !ids.insert(&event.identity)
                || event.identity.scan_id.is_empty()
                || event.sources.is_empty()
                || event.report_ids.iter().collect::<BTreeSet<_>>().len() != event.report_ids.len()
                || event.sources.iter().any(|s| !sources.contains(s))
                || event.detections.len() > 1000
            {
                return Err(invalid());
            }
            for report in &event.report_ids {
                if unique_reports
                    && report_owners
                        .insert(report, &event.identity.scan_id)
                        .is_some_and(|prior| prior != &event.identity.scan_id)
                {
                    return Err(invalid());
                }
            }
            let mut rows = BTreeSet::new();
            for d in &event.detections {
                if !event.report_ids.contains(&d.report_id)
                    || !rows.insert((&d.report_id, d.row))
                    || d.snippets.len() > 4
                {
                    return Err(invalid());
                }
                let mut families = BTreeSet::new();
                for snippet in &d.snippets {
                    if !["pi", "dlp", "dbs", "tc"].contains(&snippet.family.as_str())
                        || !families.insert(&snippet.family)
                        || snippet.excerpts.len() > 20
                        || (snippet.state == SnippetState::Present && snippet.excerpts.is_empty())
                        || (snippet.state != SnippetState::Present && !snippet.excerpts.is_empty())
                    {
                        return Err(invalid());
                    }
                }
            }
        }
        Ok(())
    }
    /// No primary source established usable observations or an explicit empty population.
    pub fn health_unknown(&self) -> bool {
        !self.sources.iter().any(|s| {
            s.required
                && !matches!(s.name.as_str(), "evidence-storage" | "archive-range")
                && s.status != Status::Unavailable
                && (s.records > 0
                    || (s.status == Status::Complete
                        && matches!(
                            s.name.as_str(),
                            "profiles" | "registered-apps" | "applications" | "sessions"
                        )))
        })
    }
    pub fn incomplete(&self) -> usize {
        self.sources
            .iter()
            .filter(|s| s.required && s.status != Status::Complete)
            .count()
    }
}
fn bounded(value: &Value, depth: usize) -> Result<(), CliError> {
    if depth > 32 {
        return Err(invalid());
    }
    match value {
        Value::String(s) if s.len() > 16384 => return Err(invalid()),
        Value::Array(a) => {
            if a.len() > 10000 {
                return Err(invalid());
            }
            for v in a {
                bounded(v, depth + 1)?;
            }
        }
        Value::Object(o) => {
            for v in o.values() {
                bounded(v, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Duplicate-rejecting JSON reader; serde_json's recursion limit remains enabled.
pub struct Strict(pub Value);
impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Strict;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("bounded JSON")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Strict, E> {
                Ok(Strict(Value::Bool(v)))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Strict, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| Strict(Value::Number(n)))
                    .ok_or_else(|| E::custom("number"))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Strict, E> {
                Ok(Strict(Value::String(v.to_owned())))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut a: A) -> Result<Strict, A::Error> {
                let mut out = Vec::new();
                while let Some(v) = a.next_element::<Strict>()? {
                    if out.len() >= 10000 {
                        return Err(serde::de::Error::custom("limit"));
                    }
                    out.push(v.0);
                }
                Ok(Strict(Value::Array(out)))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut a: A) -> Result<Strict, A::Error> {
                let mut out = serde_json::Map::new();
                while let Some(k) = a.next_key::<String>()? {
                    if out.contains_key(&k) {
                        return Err(serde::de::Error::custom("duplicate"));
                    }
                    let v = a.next_value::<Strict>()?;
                    out.insert(k, v.0);
                }
                Ok(Strict(Value::Object(out)))
            }
        }
        d.deserialize_any(Visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_primary_health_ignores_optional_and_partial_empty_sources() {
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap();
        let mut source = e.sources[0].clone();
        source.name = "application-telemetry".into();
        source.required = false;
        source.records = 0;
        for status in [Status::Complete, Status::Partial, Status::Unavailable] {
            source.status = status;
            e.sources = vec![source.clone()];
            assert!(e.health_unknown());
        }
        source.name = "sessions".into();
        source.required = true;
        source.status = Status::Partial;
        e.sources = vec![source.clone()];
        assert!(e.health_unknown());
        source.status = Status::Complete;
        e.sources = vec![source.clone()];
        assert!(!e.health_unknown());
        source.status = Status::Partial;
        source.records = 1;
        e.sources = vec![source];
        assert!(!e.health_unknown());
    }
    #[test]
    fn native_history_context_preserves_ambiguous_reports_but_public_evidence_remains_strict() {
        let mut e = Evidence::parse(include_str!(
            "../../../../tests/fixtures/signal/evidence.json"
        ))
        .unwrap();
        let mut other = e.events[0].clone();
        other.identity.scan_id = "11111111-1111-4111-8111-111111111111".into();
        e.events.push(other);
        assert!(e.seal().is_err());
        e.seal_native().unwrap();
        assert!(e.validate().is_err());
        assert!(Evidence::parse(&serde_json::to_string(&e).unwrap()).is_err());
    }
}
