use super::*;
use serde_json::{Value, json};
use std::io::{Read, Write};

const NAMES: [(&str, &str); 3] = [
    ("legacy_event_recovery_pending", "host_event_bus_pending"),
    ("legacy_credit_handoff_receipts", "credit_event_handoffs"),
    ("legacy_event_recovery_state", "host_event_bus_counter"),
];

fn lines(bytes: &[u8]) -> Vec<Value> {
    let mut text = String::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_string(&mut text)
        .unwrap();
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn encode(lines: &[Value]) -> Vec<u8> {
    let mut writer = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    for line in lines {
        writeln!(writer, "{line}").unwrap();
    }
    writer.finish().unwrap()
}
fn old_name(value: &mut Value) {
    if let Some((_, old)) = NAMES.iter().find(|(new, _)| value.as_str() == Some(*new)) {
        *value = json!(old);
    }
}
fn historical(bytes: &[u8]) -> Vec<Value> {
    let mut rows = lines(bytes);
    for table in rows[0]["tables"].as_array_mut().unwrap() {
        old_name(&mut table["name"]);
        for name in table["prerequisites"].as_array_mut().unwrap() {
            old_name(name);
        }
    }
    for dependency in rows[0]["schema_dependencies"].as_array_mut().unwrap() {
        old_name(&mut dependency["name"]);
    }
    for row in rows.iter_mut().skip(1) {
        old_name(&mut row["table"]);
    }
    rows
}
async fn seed(db: &PgPool) {
    // One receipt remains after an explicit take; another has an outstanding obligation.
    for sequence in [11i64, 12] {
        sqlx::query("insert into legacy_credit_handoff_receipts(event_id,sequence,cutover_id,origin) select $1,$2,cutover_id,'legacy_snapshot' from legacy_event_recovery_state")
            .bind(Uuid::now_v7()).bind(sequence).execute(db).await.unwrap();
    }
    sqlx::query("insert into legacy_event_recovery_pending(sequence,topic,payload,credit_event_id,value_size_bytes) select sequence,$1,$2,event_id,7 from legacy_credit_handoff_receipts where sequence=12")
        .bind(b"unknown\0topic".as_slice()).bind("{\"a\":1}").execute(db).await.unwrap();
    sqlx::query("insert into system_default_upgrade_runs(id,system_version,status,summary_json) values($1,'recovery-fixture','succeeded','{\"source\":true}'::jsonb)")
        .bind(Uuid::now_v7()).execute(db).await.unwrap();
    sqlx::query("update legacy_event_recovery_state set last_sequence=90")
        .execute(db)
        .await
        .unwrap();
}
async fn snapshot(db: &PgPool) -> Vec<Vec<Value>> {
    let mut result = Vec::new();
    // Static SQL only: these are the canonical production relations.
    for sql in [
        "select to_jsonb(t) from legacy_event_recovery_pending t order by sequence",
        "select to_jsonb(t) from legacy_credit_handoff_receipts t order by sequence",
        "select to_jsonb(t) from legacy_event_recovery_state t",
        "select to_jsonb(t) from system_default_upgrade_runs t order by id",
    ] {
        result.push(sqlx::query_scalar(sql).fetch_all(db).await.unwrap());
    }
    result
}
async fn denied_unchanged(repo: &PgSelectiveBackupRepository, db: &PgPool, rows: &[Value]) {
    // A mutable fact differs from the archive, proving rejected restore writes no earlier rows.
    sqlx::query("update system_default_upgrade_runs set summary_json='{\"target\":true}'::jsonb")
        .execute(db)
        .await
        .unwrap();
    let before = snapshot(db).await;
    let bytes = encode(rows);
    if let Ok(preview) = repo.preflight(reader(bytes.clone()), "key", "key").await {
        assert!(
            !preview.failures.is_empty(),
            "invalid archive passed preflight"
        );
    }
    assert!(
        repo.restore(reader(bytes), "key", "key", true)
            .await
            .is_err()
    );
    assert_eq!(
        snapshot(db).await,
        before,
        "rejected archive mutated recovery state"
    );
}

#[tokio::test]
async fn selective_legacy_recovery_canonical_catalog_and_old_archive_exact_roundtrip() {
    let (db, _) = fixture().await;
    seed(&db).await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let catalog = repo.catalog().await.unwrap();
    let runtime = catalog
        .iter()
        .find(|category| category.feature_id == "system.system-runtime")
        .unwrap();
    for (new, old) in NAMES {
        assert!(runtime.data_tables.iter().any(|table| table == new));
        assert!(!runtime.data_tables.iter().any(|table| table == old));
    }
    let bytes = capture(&repo, select("system-runtime", true, true)).await;
    let canonical = lines(&bytes);
    for (new, old) in NAMES {
        assert!(
            canonical[0]["tables"]
                .as_array()
                .unwrap()
                .iter()
                .any(|table| table["name"] == new)
        );
        assert!(
            !canonical[0]["tables"]
                .as_array()
                .unwrap()
                .iter()
                .any(|table| table["name"] == old)
        );
    }
    let expected = snapshot(&db).await;
    let historical = historical(&bytes);
    let mut with_dependency = historical.clone();
    let tables = with_dependency[0]["tables"].as_array_mut().unwrap();
    let state = tables.remove(
        tables
            .iter()
            .position(|table| table["name"] == "host_event_bus_counter")
            .unwrap(),
    );
    with_dependency[0]["schema_dependencies"].as_array_mut().unwrap().push(json!({
        "name":"host_event_bus_counter", "columns":state["columns"], "primary_key":state["primary_key"]
    }));
    with_dependency.retain(|row| row["table"] != "host_event_bus_counter");
    let dependency_bytes = encode(&with_dependency);
    let scratch = archive::unpack(reader(dependency_bytes.clone()))
        .await
        .unwrap();
    let header = archive::header(&mut scratch.reader().await.unwrap())
        .await
        .unwrap();
    assert!(
        header
            .schema_dependencies
            .iter()
            .any(|dependency| dependency.name == "legacy_event_recovery_state")
    );
    assert!(
        header
            .tables
            .iter()
            .find(|table| table.name == "legacy_event_recovery_pending")
            .unwrap()
            .prerequisites
            .iter()
            .any(|name| name == "legacy_credit_handoff_receipts")
    );
    assert!(
        repo.preflight(reader(dependency_bytes), "key", "key")
            .await
            .unwrap()
            .failures
            .is_empty()
    );

    sqlx::query("delete from legacy_event_recovery_pending")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("delete from legacy_credit_handoff_receipts")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("update legacy_event_recovery_state set last_sequence=0")
        .execute(&db)
        .await
        .unwrap();
    let bytes = encode(&historical);
    let preview = repo
        .preflight(reader(bytes.clone()), "key", "key")
        .await
        .unwrap();
    assert!(preview.failures.is_empty(), "{:?}", preview.failures);
    repo.restore(reader(bytes.clone()), "key", "key", true)
        .await
        .unwrap();
    repo.restore(reader(bytes), "key", "key", true)
        .await
        .unwrap();
    assert_eq!(snapshot(&db).await, expected);
    // This proves exact retained receipts, including the receipt with no pending row.
    // It does not claim consumer ACK or suppression of a taken row from older backups.
}

#[tokio::test]
async fn selective_legacy_recovery_rejects_alias_duplicates_undeclared_alias_and_duplicate_rows() {
    let (db, _) = fixture().await;
    seed(&db).await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let bytes = capture(&repo, select("system-runtime", true, true)).await;
    let old = historical(&bytes);
    let mut duplicate = old.clone();
    let mut table = duplicate[0]["tables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|table| table["name"] == "host_event_bus_pending")
        .unwrap()
        .clone();
    table["name"] = json!("legacy_event_recovery_pending");
    duplicate[0]["tables"].as_array_mut().unwrap().push(table);
    denied_unchanged(&repo, &db, &duplicate).await;

    let mut undeclared = old.clone();
    let row = undeclared
        .iter_mut()
        .skip(1)
        .find(|row| row["table"] == "host_event_bus_pending")
        .unwrap();
    row["table"] = json!("legacy_event_recovery_pending");
    denied_unchanged(&repo, &db, &undeclared).await;
    let mut duplicate = old.clone();
    let row = duplicate
        .iter()
        .skip(1)
        .find(|row| row["table"] == "host_event_bus_pending")
        .unwrap()
        .clone();
    duplicate.push(row);
    denied_unchanged(&repo, &db, &duplicate).await;

    let mut duplicate = old.clone();
    let table = duplicate[0]["tables"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|table| table["name"] == "host_event_bus_pending")
        .unwrap();
    table["prerequisites"] = json!(["credit_event_handoffs", "legacy_credit_handoff_receipts"]);
    denied_unchanged(&repo, &db, &duplicate).await;

    let mut duplicate = old;
    let state = duplicate[0]["tables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|table| table["name"] == "host_event_bus_counter")
        .unwrap();
    let dependency = json!({"name":"legacy_event_recovery_state","columns":state["columns"],"primary_key":state["primary_key"]});
    duplicate[0]["schema_dependencies"]
        .as_array_mut()
        .unwrap()
        .push(dependency);
    denied_unchanged(&repo, &db, &duplicate).await;
    // A dependency-only declaration also rejects old/new aliases of the same relation.
    let tables = duplicate[0]["tables"].as_array_mut().unwrap();
    tables.retain(|table| table["name"] != "host_event_bus_counter");
    duplicate.retain(|row| row["table"] != "host_event_bus_counter");
    let mut dependency = duplicate[0]["schema_dependencies"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    dependency["name"] = json!("host_event_bus_counter");
    duplicate[0]["schema_dependencies"]
        .as_array_mut()
        .unwrap()
        .push(dependency);
    denied_unchanged(&repo, &db, &duplicate).await;
}

#[tokio::test]
async fn selective_legacy_recovery_old_archive_preserves_immutable_rows_and_target_highwater() {
    let (db, _) = fixture().await;
    seed(&db).await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let bytes = capture(&repo, select("system-runtime", true, true)).await;
    let old = historical(&bytes);
    for (table, field, changed) in [
        (
            "host_event_bus_pending",
            "payload",
            json!("{\"different\":true}"),
        ),
        ("host_event_bus_pending", "value_size_bytes", json!(99)),
        ("credit_event_handoffs", "origin", json!("live")),
        ("credit_event_handoffs", "cutover_id", json!(Uuid::now_v7())),
    ] {
        let mut conflict = old.clone();
        conflict
            .iter_mut()
            .skip(1)
            .find(|row| row["table"] == table)
            .unwrap()["row"][field] = changed;
        denied_unchanged(&repo, &db, &conflict).await;
    }
    sqlx::query("update legacy_event_recovery_state set last_sequence=999,cutover_id=$1")
        .bind(Uuid::now_v7())
        .execute(&db)
        .await
        .unwrap();
    let expected = snapshot(&db).await;
    repo.restore(reader(encode(&old)), "key", "key", true)
        .await
        .unwrap();
    assert_eq!(
        snapshot(&db).await[..3],
        expected[..3],
        "restore rewound highwater or target cutover provenance"
    );
}
