//! Streaming framing, exact-byte integrity and bounded strict decoding.
use crate::{database, failure, identity::Cloud};
use airs_runtime_core::error::{CliError, SecretSet};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{BufRead, Read, Write},
};

pub const MAX_LINE: usize = 1024 * 1024;
pub const MAX_TOTAL: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Table {
    CollectionRuns,
    Events,
    EventReports,
    EventFacts,
    FactRuns,
    DetectorObservations,
    Receipts,
    ReceiptFacts,
    ReceiptRuns,
    Examples,
    ExampleRuns,
    ProfileObservations,
    ApplicationObservations,
    ApplicationRuns,
    ProfileRuns,
    AggregateObservations,
    AggregateRuns,
    CoverageSegments,
    SyncWork,
    WorkAttempts,
    WorkTransitions,
    ManagementReads,
    ServiceCooldowns,
}
impl Table {
    pub const ALL: [Self; 23] = [
        Self::CollectionRuns,
        Self::Events,
        Self::EventReports,
        Self::EventFacts,
        Self::FactRuns,
        Self::DetectorObservations,
        Self::Receipts,
        Self::ReceiptFacts,
        Self::ReceiptRuns,
        Self::Examples,
        Self::ExampleRuns,
        Self::ProfileObservations,
        Self::ApplicationObservations,
        Self::ApplicationRuns,
        Self::ProfileRuns,
        Self::AggregateObservations,
        Self::AggregateRuns,
        Self::CoverageSegments,
        Self::SyncWork,
        Self::WorkAttempts,
        Self::WorkTransitions,
        Self::ManagementReads,
        Self::ServiceCooldowns,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::CollectionRuns => "collection_runs",
            Self::Events => "events",
            Self::EventReports => "event_reports",
            Self::EventFacts => "event_facts",
            Self::FactRuns => "fact_runs",
            Self::DetectorObservations => "detector_observations",
            Self::Receipts => "receipts",
            Self::ReceiptFacts => "receipt_facts",
            Self::ReceiptRuns => "receipt_runs",
            Self::Examples => "examples",
            Self::ExampleRuns => "example_runs",
            Self::ProfileObservations => "profile_observations",
            Self::ApplicationObservations => "application_observations",
            Self::ApplicationRuns => "application_runs",
            Self::ProfileRuns => "profile_runs",
            Self::AggregateObservations => "aggregate_observations",
            Self::AggregateRuns => "aggregate_runs",
            Self::CoverageSegments => "coverage_segments",
            Self::SyncWork => "sync_work",
            Self::WorkAttempts => "work_attempts",
            Self::WorkTransitions => "work_transitions",
            Self::ManagementReads => "management_reads",
            Self::ServiceCooldowns => "service_cooldowns",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
    pub format: String,
    pub format_version: u64,
    pub schema_version: i64,
    pub schema_digest: String,
    pub analysis_version: u64,
    pub redaction_version: u64,
    pub cloud: Cloud,
    pub exported_ms: i64,
}
impl Header {
    pub fn new(cloud: &Cloud, exported_ms: i64) -> Self {
        Self {
            format: "airs-runtime-history".into(),
            format_version: 1,
            schema_version: database::VERSION,
            schema_digest: format!("{:x}", Sha256::digest(database::SCHEMA)),
            analysis_version: 1,
            redaction_version: 1,
            cloud: cloud.clone(),
            exported_ms,
        }
    }
    pub fn validate(&self, cloud: &Cloud, secrets: &SecretSet) -> Result<(), CliError> {
        if self.format != "airs-runtime-history"
            || self.format_version != 1
            || self.schema_version != database::VERSION
            || self.schema_digest != format!("{:x}", Sha256::digest(database::SCHEMA))
            || self.analysis_version != 1
            || self.redaction_version != 1
            || &self.cloud != cloud
        {
            return Err(failure(
                "logical transfer version, schema or explicit cloud binding does not match",
            ));
        }
        self.cloud.check_public(secrets)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub table: Table,
    pub values: Vec<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Footer {
    pub counts: BTreeMap<Table, u64>,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Line {
    Header(Header),
    Row(Row),
    Footer(Footer),
}
fn invalid() -> CliError {
    failure("logical transfer is malformed, unsupported, inconsistent or exceeds its bounds")
}
fn counts() -> BTreeMap<Table, u64> {
    Table::ALL.into_iter().map(|t| (t, 0)).collect()
}
fn encoded(line: &Line) -> Result<Vec<u8>, CliError> {
    let mut bytes = serde_json::to_vec(line).map_err(|_| invalid())?;
    bytes.push(b'\n');
    if bytes.len() > MAX_LINE {
        return Err(invalid());
    }
    Ok(bytes)
}

pub struct Encoder<W: Write> {
    writer: W,
    hasher: Sha256,
    counts: BTreeMap<Table, u64>,
    bytes: u64,
}
impl<W: Write> Encoder<W> {
    pub fn new(mut writer: W, header: Header) -> Result<Self, CliError> {
        let bytes = encoded(&Line::Header(header))?;
        writer
            .write_all(&bytes)
            .map_err(|_| failure("cannot write private logical transfer"))?;
        Ok(Self {
            writer,
            hasher: Sha256::new_with_prefix(&bytes),
            counts: counts(),
            bytes: bytes.len() as u64,
        })
    }
    pub fn row(&mut self, row: Row) -> Result<(), CliError> {
        let table = row.table;
        let bytes = encoded(&Line::Row(row))?;
        // Leave enough space for a maximum-size footer before publication.
        if self
            .bytes
            .checked_add(bytes.len() as u64 + MAX_LINE as u64)
            .is_none_or(|n| n > MAX_TOTAL)
        {
            return Err(invalid());
        }
        self.writer
            .write_all(&bytes)
            .map_err(|_| failure("cannot write private logical transfer"))?;
        self.hasher.update(&bytes);
        self.bytes += bytes.len() as u64;
        *self.counts.get_mut(&table).expect("fixed transfer table") += 1;
        Ok(())
    }
    pub fn finish(mut self) -> Result<(W, Footer, u64), CliError> {
        let footer = Footer {
            counts: self.counts,
            sha256: format!("{:x}", self.hasher.finalize()),
        };
        let bytes = encoded(&Line::Footer(footer.clone()))?;
        self.writer
            .write_all(&bytes)
            .map_err(|_| failure("cannot finish private logical transfer"))?;
        self.bytes += bytes.len() as u64;
        Ok((self.writer, footer, self.bytes))
    }
}

pub struct Decoder<R: BufRead> {
    reader: R,
    hasher: Sha256,
    counts: BTreeMap<Table, u64>,
    pub bytes: u64,
    started: bool,
    finished: bool,
}
impl<R: BufRead> Decoder<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            hasher: Sha256::new(),
            counts: counts(),
            bytes: 0,
            started: false,
            finished: false,
        }
    }
    pub fn next(&mut self) -> Result<Option<Line>, CliError> {
        if self.finished {
            return Ok(None);
        }
        let mut bytes = Vec::new();
        Read::take(&mut self.reader, MAX_LINE as u64 + 1)
            .read_until(b'\n', &mut bytes)
            .map_err(|_| failure("cannot read logical transfer input"))?;
        if bytes.is_empty() || bytes.len() > MAX_LINE || !bytes.ends_with(b"\n") {
            return Err(invalid());
        }
        self.bytes = self
            .bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(invalid)?;
        if self.bytes > MAX_TOTAL {
            return Err(invalid());
        }
        let strict: airs_runtime_core::signal::evidence::Strict =
            serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        let line: Line = serde_json::from_value(strict.0).map_err(|_| invalid())?;
        match &line {
            Line::Header(_) if !self.started => {
                self.started = true;
                self.hasher.update(&bytes);
            }
            Line::Row(row) if self.started => {
                if row.values.len() > 32
                    || row
                        .values
                        .iter()
                        .any(|v| !v.is_null() && !v.is_string() && v.as_i64().is_none())
                {
                    return Err(invalid());
                }
                *self.counts.get_mut(&row.table).ok_or_else(invalid)? += 1;
                self.hasher.update(&bytes);
            }
            Line::Footer(footer) if self.started => {
                if footer.counts != self.counts
                    || footer.sha256 != format!("{:x}", self.hasher.clone().finalize())
                    || !self.reader.fill_buf().map_err(|_| invalid())?.is_empty()
                {
                    return Err(invalid());
                }
                self.finished = true;
            }
            _ => return Err(invalid()),
        }
        Ok(Some(line))
    }
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
