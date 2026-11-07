use super::*;
use std::io::Cursor;
fn header() -> Header {
    let cloud = Cloud::from_tenant(&airs_runtime_core::tenant::TenantContext {
        selected: airs_runtime_core::tenant::Selected {
            name: "fixture".into(),
            tsg_id: "100".into(),
            by_flag: false,
        },
        config: airs_runtime_core::tenant::config::TenantConfig::default(),
    })
    .unwrap();
    Header::new(&cloud, 1)
}
fn fixture() -> Vec<u8> {
    let mut encoder = Encoder::new(Vec::new(), header()).unwrap();
    encoder
        .row(Row {
            table: Table::CollectionRuns,
            values: vec![
                Value::String("synthetic-run".into()),
                Value::Number(1.into()),
                Value::Null,
            ],
        })
        .unwrap();
    let (bytes, footer, size) = encoder.finish().unwrap();
    assert_eq!(size, bytes.len() as u64);
    assert_eq!(footer.counts[&Table::CollectionRuns], 1);
    bytes
}
fn valid(bytes: &[u8]) -> bool {
    let mut reader = Decoder::new(Cursor::new(bytes));
    loop {
        match reader.next() {
            Ok(Some(_)) => {}
            Ok(None) => return true,
            Err(_) => return false,
        }
    }
}
#[test]
fn logical_transfer_exact_bytes_round_trip_and_explicit_cloud_versions() {
    let bytes = fixture();
    let mut reader = Decoder::new(Cursor::new(&bytes));
    let Some(Line::Header(h)) = reader.next().unwrap() else {
        panic!("header")
    };
    h.validate(&header().cloud, &SecretSet::default()).unwrap();
    let Some(Line::Row(r)) = reader.next().unwrap() else {
        panic!("row")
    };
    assert_eq!(r.table, Table::CollectionRuns);
    let Some(Line::Footer(footer)) = reader.next().unwrap() else {
        panic!("footer")
    };
    assert_eq!(footer.counts.len(), Table::ALL.len());
    assert_eq!(reader.bytes, bytes.len() as u64);
    assert!(reader.next().unwrap().is_none());
    for field in [
        "format_version",
        "schema_version",
        "analysis_version",
        "redaction_version",
    ] {
        let mut tree = serde_json::to_value(header()).unwrap();
        tree[field] = Value::from(999);
        let h: Header = serde_json::from_value(tree).unwrap();
        assert!(h.validate(&header().cloud, &SecretSet::default()).is_err());
    }
    let mut wrong = header().cloud;
    wrong.tsg = "101".into();
    assert!(header().validate(&wrong, &SecretSet::default()).is_err());
}
#[test]
fn logical_transfer_rejects_digest_counts_duplicate_keys_kinds_and_trailing_content() {
    let bytes = fixture();
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(!valid(
        text.replace("synthetic-run", "different-run").as_bytes()
    ));
    assert!(!valid(
        text.replace("\"collection_runs\":1", "\"collection_runs\":2")
            .as_bytes()
    ));
    assert!(!valid(
        text.replace(
            "\"kind\":\"header\"",
            "\"kind\":\"header\",\"kind\":\"header\""
        )
        .as_bytes()
    ));
    assert!(!valid(
        text.replace("\"table\":\"collection_runs\"", "\"table\":\"execute_sql\"")
            .as_bytes()
    ));
    assert!(!valid(format!("{text}\n").as_bytes()));
    assert!(!valid(
        text.lines()
            .take(2)
            .collect::<Vec<_>>()
            .join("\n")
            .as_bytes()
    ));
    assert!(!valid(&bytes[..bytes.len() - 1]));
    assert!(!valid(b"SQLite format 3\0"));
    assert!(!valid(b"SELECT * FROM events;\n"));
    let first = text.lines().next().unwrap();
    assert!(!valid(format!("{first}\n{first}\n").as_bytes()));
}
#[test]
fn logical_transfer_line_bound_and_scalar_types_do_not_allow_unbounded_or_executable_data() {
    assert!(!valid(&vec![b'x'; MAX_LINE + 1]));
    let mut encoder = Encoder::new(Vec::new(), header()).unwrap();
    assert!(
        encoder
            .row(Row {
                table: Table::Events,
                values: vec![Value::String("x".repeat(MAX_LINE))]
            })
            .is_err()
    );
    for value in [
        serde_json::json!(1.5),
        serde_json::json!(true),
        serde_json::json!({"sql":"DROP TABLE events"}),
        serde_json::json!([1]),
    ] {
        let mut encoder = Encoder::new(Vec::new(), header()).unwrap();
        encoder
            .row(Row {
                table: Table::Events,
                values: vec![value],
            })
            .unwrap();
        assert!(!valid(&encoder.finish().unwrap().0));
    }
}
#[test]
/// Every archive table is transferred except `archive_meta` (the destination's own
/// identity) and `pruned_examples`: tombstones record this archive's own erasures; they
/// are not history data, and carrying them would make another archive refuse its own
/// examples. `archive_generation` and `import_operations` are the destination's own incarnation
/// and import journal.
fn logical_transfer_table_kinds_cover_every_trusted_schema_table_except_archive_meta() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(database::SCHEMA).unwrap();
    let mut names=conn.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT IN ('archive_meta','pruned_examples','archive_generation','import_operations') ORDER BY name").unwrap().query_map([],|r|r.get::<_,String>(0)).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
    names.sort();
    let mut kinds = Table::ALL.map(|t| t.name().to_owned());
    kinds.sort();
    assert_eq!(names, kinds);
}
