use super::*;
use serde_json::{Value, json};
use std::io::{Read, Write};

const LEDGER: &str = "legacy_event_recovery_ledger";
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
fn historical(bytes: &[u8], original: bool) -> Vec<Value> {
    let canonical = lines(bytes);
    let mut header = canonical[0].clone();
    header["tables"]
        .as_array_mut()
        .unwrap()
        .retain(|t| t["name"] != LEDGER);
    let mut output = vec![];
    for (new, old) in NAMES {
        let name = if original { old } else { new };
        let pk = if new.ends_with("pending") {
            "sequence"
        } else if new.ends_with("receipts") {
            "event_id"
        } else {
            "singleton"
        };
        let prerequisites = if new.ends_with("pending") {
            vec![if original {
                "credit_event_handoffs"
            } else {
                "legacy_credit_handoff_receipts"
            }]
        } else {
            vec![]
        };
        header["tables"].as_array_mut().unwrap().push(json!({"name":name,"feature_id":"system.system-runtime","data":true,"prerequisite":false,"columns":super::super::recovery_archive::columns(new),"primary_key":[pk],"plugin_owners":[],"prerequisites":prerequisites}));
    }
    output.push(header);
    for record in canonical.into_iter().skip(1) {
        if record["table"] != LEDGER {
            output.push(record);
            continue;
        }
        let row = &record["row"];
        let name = |index: usize| {
            if original {
                NAMES[index].1
            } else {
                NAMES[index].0
            }
        };
        if row["sequence"] == 0 {
            output.push(json!({"table":name(2),"row":{"singleton":true,"last_sequence":row["last_sequence"],"cutover_id":row["cutover_id"],"created_at":row["created_at"]}}));
            continue;
        }
        if row["disposition"] == "pending" {
            output.push(json!({"table":name(0),"row":{"sequence":row["sequence"],"topic":row["topic"],"payload":row["payload"],"credit_event_id":row["credit_event_id"],"value_size_bytes":row["value_size_bytes"]}}));
        }
        if !row["credit_event_id"].is_null() {
            output.push(json!({"table":name(1),"row":{"event_id":row["credit_event_id"],"sequence":row["sequence"],"cutover_id":row["cutover_id"],"origin":row["origin"],"handed_off_at":row["handed_off_at"]}}));
        }
    }
    output
}
async fn seed(db: &PgPool) {
    sqlx::query("insert into legacy_event_recovery_ledger(sequence,disposition,credit_event_id,cutover_id,origin,handed_off_at) select 11,'taken',$1,cutover_id,'legacy_snapshot',now() from legacy_event_recovery_ledger where sequence=0")
        .bind(Uuid::now_v7()).execute(db).await.unwrap();
    sqlx::query("insert into legacy_event_recovery_ledger(sequence,disposition,topic,payload,payload_digest,value_size_bytes,credit_event_id,cutover_id,origin,handed_off_at) select 12,'pending',$1,$2,sha256(convert_to($2,'UTF8')),7,$3,cutover_id,'legacy_snapshot',now() from legacy_event_recovery_ledger where sequence=0")
        .bind(b"unknown\0topic".as_slice()).bind("{\"a\":1}").bind(Uuid::now_v7()).execute(db).await.unwrap();
    let payload = "{\"escaped\":\"\\u0000\", \"number\": 1e+09}";
    sqlx::query("insert into legacy_event_recovery_ledger(sequence,disposition,topic,payload,payload_digest,value_size_bytes) values(13,'pending',$1,$2,sha256(convert_to($2,'UTF8')),$3)")
        .bind(b"generic\0topic".as_slice()).bind(payload).bind(payload.len() as i64).execute(db).await.unwrap();
    sqlx::query("insert into system_default_upgrade_runs(id,system_version,status,summary_json) values($1,'recovery-fixture','succeeded','{\"source\":true}'::jsonb)")
        .bind(Uuid::now_v7()).execute(db).await.unwrap();
    sqlx::query("update legacy_event_recovery_ledger set last_sequence=90 where sequence=0")
        .execute(db)
        .await
        .unwrap();
}
async fn snapshot(db: &PgPool) -> Vec<Vec<Value>> {
    let mut result = Vec::new();
    for sql in [
        "select to_jsonb(t) from legacy_event_recovery_ledger t order by sequence",
        "select to_jsonb(t) from system_default_upgrade_runs t order by id",
    ] {
        result.push(sqlx::query_scalar(sql).fetch_all(db).await.unwrap());
    }
    result
}
async fn denied_unchanged(repo: &PgSelectiveBackupRepository, db: &PgPool, rows: &[Value]) {
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
        "rejected archive partially mutated target"
    );
}
#[tokio::test]
async fn selective_legacy_recovery_canonical_catalog_and_both_historical_schemas_roundtrip() {
    let (db, _) = fixture().await;
    seed(&db).await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let categories = repo.catalog().await.unwrap();
    let runtime = categories
        .iter()
        .find(|c| c.feature_id == "system.system-runtime")
        .unwrap();
    assert!(runtime.data_tables.iter().any(|t| t == LEDGER));
    for (new, old) in NAMES {
        assert!(!runtime.data_tables.iter().any(|t| t == new || t == old));
    }
    let bytes = capture(&repo, select("system-runtime", true, true)).await;
    let canonical = lines(&bytes);
    assert!(
        canonical[0]["tables"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == LEDGER)
    );
    for rows in [
        canonical,
        historical(&bytes, true),
        historical(&bytes, false),
    ] {
        let (target, _) = fixture().await;
        let target_repo = PgSelectiveBackupRepository::new(target.clone());
        let archive = encode(&rows);
        let preview = target_repo
            .preflight(reader(archive.clone()), "key", "key")
            .await
            .unwrap();
        assert!(preview.failures.is_empty(), "{:?}", preview.failures);
        for _ in 0..2 {
            target_repo
                .restore(reader(archive.clone()), "key", "key", true)
                .await
                .unwrap();
        }
        assert_eq!(snapshot(&target).await, snapshot(&db).await);
    }
}
#[tokio::test]
async fn selective_legacy_recovery_rejects_alias_duplicates_undeclared_alias_and_duplicate_rows() {
    let (db, _) = fixture().await;
    seed(&db).await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let bytes = capture(&repo, select("system-runtime", true, true)).await;
    let old = historical(&bytes, true);
    let mut bad = old.clone();
    let mut table = bad[0]["tables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "host_event_bus_pending")
        .unwrap()
        .clone();
    table["name"] = json!("legacy_event_recovery_pending");
    bad[0]["tables"].as_array_mut().unwrap().push(table);
    denied_unchanged(&repo, &db, &bad).await;
    let mut bad = old.clone();
    bad.iter_mut()
        .skip(1)
        .find(|r| r["table"] == "host_event_bus_pending")
        .unwrap()["table"] = json!("legacy_event_recovery_pending");
    denied_unchanged(&repo, &db, &bad).await;
    for name in [
        "host_event_bus_pending",
        "credit_event_handoffs",
        "host_event_bus_counter",
    ] {
        let mut bad = old.clone();
        let row = bad
            .iter()
            .skip(1)
            .find(|r| r["table"] == name)
            .unwrap()
            .clone();
        bad.push(row);
        denied_unchanged(&repo, &db, &bad).await;
    }
    let mut bad = old.clone();
    bad[0]["tables"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|t| t["name"] == "host_event_bus_pending")
        .unwrap()["prerequisites"] =
        json!(["credit_event_handoffs", "legacy_credit_handoff_receipts"]);
    denied_unchanged(&repo, &db, &bad).await;
    let mut bad = old.clone();
    let state = bad[0]["tables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "host_event_bus_counter")
        .unwrap()
        .clone();
    bad[0]["schema_dependencies"].as_array_mut().unwrap().push(json!({"name":"legacy_event_recovery_state","columns":state["columns"],"primary_key":state["primary_key"]}));
    denied_unchanged(&repo, &db, &bad).await;
    let mut bad = old.clone();
    bad[0]["tables"]
        .as_array_mut()
        .unwrap()
        .retain(|t| t["name"] != "credit_event_handoffs");
    bad.retain(|r| r["table"] != "credit_event_handoffs");
    denied_unchanged(&repo, &db, &bad).await;
    let mut bad = old;
    let table = lines(&bytes)[0]["tables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == LEDGER)
        .unwrap()
        .clone();
    bad[0]["tables"].as_array_mut().unwrap().push(table);
    denied_unchanged(&repo, &db, &bad).await;
}
#[tokio::test]
async fn selective_legacy_recovery_missing_receipt_or_changed_identity_and_body_fail_atomically() {
    let (db, _) = fixture().await;
    seed(&db).await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let bytes = capture(&repo, select("system-runtime", true, true)).await;
    let old = historical(&bytes, true);
    for (table, field, changed) in [
        (
            "host_event_bus_pending",
            "payload",
            json!("{\"different\":true}"),
        ),
        ("host_event_bus_pending", "value_size_bytes", json!(99)),
        ("credit_event_handoffs", "origin", json!("live")),
        ("credit_event_handoffs", "cutover_id", json!(Uuid::now_v7())),
        ("credit_event_handoffs", "event_id", json!(Uuid::now_v7())),
    ] {
        let mut bad = old.clone();
        bad.iter_mut()
            .skip(1)
            .find(|r| r["table"] == table && r["row"]["sequence"] == 12)
            .unwrap()["row"][field] = changed;
        denied_unchanged(&repo, &db, &bad).await;
    }
    let mut bad = old.clone();
    bad.retain(|r| !(r["table"] == "credit_event_handoffs" && r["row"]["sequence"] == 12));
    denied_unchanged(&repo, &db, &bad).await;
    let mut bad = lines(&bytes);
    bad.iter_mut()
        .skip(1)
        .find(|r| r["table"] == LEDGER && r["row"]["sequence"] == 12)
        .unwrap()["row"]["payload_digest"] = json!(format!("\\x{}", "00".repeat(32)));
    denied_unchanged(&repo, &db, &bad).await;
    let mut bad = lines(&bytes);
    bad.retain(|r| !(r["table"] == LEDGER && r["row"]["sequence"] == 0));
    denied_unchanged(&repo, &db, &bad).await;
}
#[tokio::test]
async fn selective_legacy_recovery_taken_tombstones_survive_restart_and_old_backups_never_refill() {
    let (db, _) = fixture().await;
    seed(&db).await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let bytes = capture(&repo, select("system-runtime", true, true)).await;
    let recovery = crate::PgLegacyEventRecovery::new(db.clone());
    assert_eq!(
        recovery.poll("unknown\0topic").await.unwrap(),
        Some(json!({"a":1}))
    );
    assert_eq!(
        recovery.poll("generic\0topic").await.unwrap(),
        Some(json!({"escaped":"\0","number":1e9}))
    );
    let before = snapshot(&db).await;
    for rows in [
        lines(&bytes),
        historical(&bytes, true),
        historical(&bytes, false),
    ] {
        repo.restore(reader(encode(&rows)), "key", "key", true)
            .await
            .unwrap();
        assert_eq!(snapshot(&db).await[0], before[0]);
        let reopened = crate::PgLegacyEventRecovery::new(db.clone());
        assert!(reopened.list_ephemeral_entries().await.unwrap().is_empty());
        assert!(reopened.poll("generic\0topic").await.unwrap().is_none());
    }
    let (target, _) = fixture().await;
    let taken = capture(&repo, select("system-runtime", true, true)).await;
    PgSelectiveBackupRepository::new(target.clone())
        .restore(reader(taken), "key", "key", true)
        .await
        .unwrap();
    assert_eq!(snapshot(&target).await[0], before[0]);
    // Pre-ledger generic takes had no receipt: absence below high-water cannot
    // distinguish an old take from a missing entry, so reject instead of revive.
    sqlx::query("delete from legacy_event_recovery_ledger where sequence=13")
        .execute(&db)
        .await
        .unwrap();
    denied_unchanged(&repo, &db, &historical(&bytes, true)).await;
}
#[tokio::test]
async fn selective_legacy_recovery_state_merge_preserves_target_highwater_and_cutover() {
    let (db, _) = fixture().await;
    seed(&db).await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let bytes = capture(&repo, select("system-runtime", true, true)).await;
    sqlx::query(
        "update legacy_event_recovery_ledger set last_sequence=999,cutover_id=$1 where sequence=0",
    )
    .bind(Uuid::now_v7())
    .execute(&db)
    .await
    .unwrap();
    let expected = snapshot(&db).await;
    repo.restore(
        reader(encode(&historical(&bytes, true))),
        "key",
        "key",
        true,
    )
    .await
    .unwrap();
    assert_eq!(snapshot(&db).await[0], expected[0]);
}
