//! The M5 exit gate's adversarial transfer cases, shared by every adapter (M5 review 3
//! finding 7): the store driver (`history_transfer_adversarial.rs`) and the MCP service
//! adapters (admission, chunks, finalize, import execution, in `airs-mcp`'s
//! `tools.rs` tests) run exactly these cases. Each case is built from a valid export
//! and re-encoded with valid framing, counts and checksum, so a refusal must come from
//! validation rather than a stale trailer. Pure data: no adapter, sandbox or archive.
#![allow(dead_code)]

use rusqlite::Connection;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const SCHEMA: &str = include_str!("../../src/schema.sql");

/// The cases a valid export yields, each `(name, bytes)`; every one must be refused
/// before any destination change, through every adapter, into empty and populated
/// destinations.
pub fn adversarial(export: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    let lines = lines(export);
    let mut cases = Vec::new();
    let mut duplicate = lines.clone();
    let r = row(&duplicate, "collection_runs");
    duplicate.insert(r + 1, duplicate[r].clone());
    cases.push(("duplicate-primary-key", encode(duplicate)));
    let mut cardinality = lines.clone();
    let e = row(&lines, "events");
    cardinality[e]["values"].as_array_mut().unwrap().pop();
    cases.push(("wrong-column-cardinality", encode(cardinality)));
    let mut orphan = lines.clone();
    let f = row(&orphan, "fact_runs");
    orphan[f]["values"][column("fact_runs", "fact_id")] = json!(format!(
        "{:x}",
        Sha256::digest(b"synthetic-fact-absent-from-transfer")
    ));
    cases.push(("orphan-fact-run", encode(orphan)));
    let mut projection = lines.clone();
    projection[e]["values"][column("events", "verdict")] = json!("benign");
    cases.push(("forged-benign-projection", encode(projection)));
    let mut erased = lines.clone();
    for name in ["app_id", "app_name", "session_id"] {
        erased[e]["values"][column("events", name)] = json!("");
    }
    cases.push(("erased-identity", encode(erased)));
    if let Some(x) = lines
        .iter()
        .position(|l| l["kind"] == "row" && l["table"] == "examples")
    {
        let mut unowned = lines.clone();
        unowned[x]["values"][column("examples", "report_id")] = json!("unowned-report");
        cases.push(("unowned-example", encode(unowned)));
    }
    let mut work = lines.clone();
    let footer = work.pop().unwrap();
    work.push(json!({"kind": "row", "table": "sync_work", "values": [
        "work-forged", "reports", json!({"identity": {"scan_id": []}}).to_string(),
        "pending", null, 0, 0, null, null, null, "{}"]}));
    work.push(footer);
    cases.push(("array-work-scan-id", encode(work)));
    for (name, id) in [
        ("forged-receipt-empty-id", ""),
        ("forged-receipt", "forged"),
    ] {
        let mut forged = lines.clone();
        let footer = forged.pop().unwrap();
        let mut values = vec![Value::Null; width("receipts")];
        values[column("receipts", "receipt_id")] = json!(id);
        values[column("receipts", "origin_key")] = json!("forged-origin");
        values[column("receipts", "first_seen_ms")] = json!(1);
        values[column("receipts", "last_seen_ms")] = json!(1);
        values[column("receipts", "binding_json")] = json!("{\"identity_withheld\":false}");
        values[column("receipts", "state")] = json!("promoted");
        values[column("receipts", "promoted_event_id")] =
            lines[e]["values"][column("events", "event_id")].clone();
        forged.push(json!({"kind": "row", "table": "receipts", "values": values}));
        forged.push(footer);
        cases.push((name, encode(forged)));
    }
    let mut wrong_cloud = lines.clone();
    wrong_cloud[0]["cloud"]["tsg"] = json!("999");
    cases.push(("wrong-cloud", encode(wrong_cloud)));
    let mut damaged = export.to_vec();
    let middle = damaged.len() / 2;
    damaged[middle] ^= 0x01;
    cases.push(("damaged-digest", damaged));
    cases.push(("truncated", export[..export.len() / 2].to_vec()));
    let mut trailing = export.to_vec();
    trailing.extend_from_slice(b"\n{}\n");
    cases.push(("trailing-bytes", trailing));
    cases
}

/// The lines of a logical transfer (header, rows, footer).
pub fn lines(bytes: &[u8]) -> Vec<Value> {
    std::str::from_utf8(bytes)
        .unwrap()
        .split_terminator('\n')
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// Re-encode with valid per-table counts and the exact-byte checksum.
pub fn encode(mut lines: Vec<Value>) -> Vec<u8> {
    let mut footer = lines.pop().unwrap();
    let mut counts = footer["counts"].as_object().unwrap().clone();
    for c in counts.values_mut() {
        *c = json!(0);
    }
    let mut bytes = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            let t = line["table"].as_str().unwrap();
            let c = counts.get_mut(t).expect("an exported table");
            *c = json!(c.as_u64().unwrap() + 1);
        }
        serde_json::to_writer(&mut bytes, line).unwrap();
        bytes.push(b'\n');
    }
    footer["counts"] = Value::Object(counts);
    footer["sha256"] = json!(format!("{:x}", Sha256::digest(&bytes)));
    serde_json::to_writer(&mut bytes, &footer).unwrap();
    bytes.push(b'\n');
    bytes
}

fn row(lines: &[Value], table: &str) -> usize {
    lines
        .iter()
        .position(|l| l["kind"] == "row" && l["table"] == table)
        .unwrap_or_else(|| panic!("the export has no {table} row"))
}

fn table_columns(table: &str) -> Vec<String> {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(SCHEMA).unwrap();
    let mut st = c
        .prepare("SELECT name FROM pragma_table_info(?1) ORDER BY cid")
        .unwrap();
    st.query_map([table], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

/// A column's position in a table's exported values (the schema's column order).
pub fn column(table: &str, name: &str) -> usize {
    table_columns(table)
        .iter()
        .position(|c| c == name)
        .unwrap_or_else(|| panic!("no {table}.{name}"))
}

fn width(table: &str) -> usize {
    table_columns(table).len()
}
