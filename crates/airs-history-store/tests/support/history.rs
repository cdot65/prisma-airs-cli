use airs_test_harness::Sandbox;
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
};

pub fn archive(s: &Sandbox) -> (PathBuf, PathBuf) {
    let config: toml::Table =
        toml::from_str(&fs::read_to_string(s.store_tenant_path("prod")).unwrap()).unwrap();
    let endpoints = ["mgmtEndpoint", "mgmtDashboardEndpoint", "mgmtTokenEndpoint"].map(|key| {
        url::Url::parse(config[key].as_str().unwrap())
            .unwrap()
            .as_str()
            .trim_end_matches('/')
            .to_owned()
    });
    let env = format!(
        "env1:{:x}",
        Sha256::digest(serde_json::to_vec(&endpoints).unwrap())
    );
    let tsg = config["mgmtTsgId"].as_str().unwrap();
    let cloud = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(1, tsg, &env)).unwrap())
    );
    let root = s.tmp().join("history");
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let dir = root.join(&cloud);
    fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
    for name in ["writer.lock", "maintenance.lock", "archive.sqlite3"] {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(dir.join(name))
            .unwrap();
    }
    let conn = Connection::open(dir.join("archive.sqlite3")).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA secure_delete=ON;").unwrap();
    conn.execute_batch(include_str!("../../src/schema.sql"))
        .unwrap();
    conn.execute(
        "INSERT INTO archive_meta VALUES (1,?1,?2,?3,?4,1,1,1,NULL)",
        params![cloud, tsg, env, serde_json::to_string(&endpoints).unwrap()],
    )
    .unwrap();
    conn.execute("INSERT INTO archive_generation VALUES (1, 7)", [])
        .unwrap();
    conn.execute_batch("INSERT INTO collection_runs VALUES ('run','sync',1,2,0,100,'partial','{}');
        INSERT INTO events VALUES ('event','app','App','session','00000000-0000-4000-8000-000000000001',0,NULL,1,1,2,NULL,'malicious','allow',0);
        INSERT INTO coverage_segments VALUES ('segment','run','app','App','sessions',0,100,2,'partial','{}');
        INSERT INTO receipts VALUES ('receipt','bulk/job/item',1,1,NULL,NULL,NULL,NULL,NULL,NULL,'{\"identity_withheld\":false}','partial',NULL);").unwrap();
    drop(conn);
    (root, dir)
}
