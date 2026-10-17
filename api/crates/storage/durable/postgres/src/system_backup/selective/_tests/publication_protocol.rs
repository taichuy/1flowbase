use super::*;
use serde_json::{Value, json};
use std::io::{Read, Write};

fn rewrite(bytes: &[u8], mut change: impl FnMut(usize, &mut Value)) -> Vec<u8> {
    let mut text = String::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_string(&mut text)
        .unwrap();
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    for (position, line) in text.lines().enumerate() {
        let mut value: Value = serde_json::from_str(line).unwrap();
        change(position, &mut value);
        writeln!(encoder, "{value}").unwrap();
    }
    encoder.finish().unwrap()
}

fn historical(bytes: &[u8]) -> Vec<u8> {
    rewrite(bytes, |position, value| {
        if position == 0 {
            for table in value["tables"].as_array_mut().unwrap() {
                if table["name"] == "lifecycle_outbox" {
                    table["columns"].as_array_mut().unwrap().retain(|column| {
                        column["name"] != "native_publication_version"
                            && column["name"] != "legacy_handoff_sequence"
                            && column["name"] != "notification_payload_digest"
                    });
                }
            }
        } else if value["table"] == "lifecycle_outbox" {
            let row = value["row"].as_object_mut().unwrap();
            row.remove("native_publication_version");
            row.remove("legacy_handoff_sequence");
            row.remove("notification_payload_digest");
        }
    })
}

async fn source(db: &PgPool, version: i16, receipt: Option<i64>) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("insert into lifecycle_outbox(event_id,transaction_id,contract_id,contract_version,canonical_payload,occurred_at,native_publication_version,legacy_handoff_sequence) values($1,$2,'model_definition.committed','1',$3,now(),$4,$5)")
        .bind(id).bind(Uuid::now_v7()).bind(b"original immutable fact".as_slice())
        .bind(version).bind(receipt).execute(db).await.unwrap();
    id
}

async fn metadata(db: &PgPool, id: Uuid) -> (i16, Option<i64>, Vec<u8>) {
    sqlx::query_as("select native_publication_version,legacy_handoff_sequence,canonical_payload from lifecycle_outbox where event_id=$1")
        .bind(id).fetch_one(db).await.unwrap()
}

async fn denied(repo: &PgSelectiveBackupRepository, bytes: Vec<u8>) {
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
}

#[tokio::test]
async fn selective_publication_protocol_old_archive_restores_original_handoff() {
    let (db, _) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let id = source(&db, 1, None).await;
    let bytes = historical(&capture(&repo, select("extension-center", true, true)).await);
    sqlx::query("delete from lifecycle_outbox")
        .execute(&db)
        .await
        .unwrap();
    let preview = repo
        .preflight(reader(bytes.clone()), "key", "key")
        .await
        .unwrap();
    assert!(preview.failures.is_empty(), "{:?}", preview.failures);
    repo.restore(reader(bytes), "key", "key", true)
        .await
        .unwrap();
    assert_eq!(
        metadata(&db, id).await,
        (1, None, b"original immutable fact".to_vec())
    );
}

#[tokio::test]
async fn selective_publication_protocol_roundtrips_both_versions_and_receipts() {
    let (db, _) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let mut expected = Vec::new();
    for (version, receipt) in [(1, None), (1, Some(101)), (2, None), (2, Some(202))] {
        let id = source(&db, version, receipt).await;
        expected.push((id, metadata(&db, id).await));
    }
    let bytes = capture(&repo, select("extension-center", true, true)).await;
    sqlx::query("delete from lifecycle_outbox")
        .execute(&db)
        .await
        .unwrap();
    repo.restore(reader(bytes.clone()), "key", "key", true)
        .await
        .unwrap();
    repo.restore(reader(bytes), "key", "key", true)
        .await
        .unwrap();
    for (id, value) in expected {
        assert_eq!(metadata(&db, id).await, value);
    }
}

#[tokio::test]
async fn selective_publication_protocol_restore_cannot_rewind_receipt() {
    let (db, _) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let id = source(&db, 1, None).await;
    let bytes = capture(&repo, select("extension-center", true, true)).await;
    sqlx::query("update lifecycle_outbox set legacy_handoff_sequence=99 where event_id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    for archive in [bytes.clone(), historical(&bytes)] {
        repo.restore(reader(archive), "key", "key", true)
            .await
            .unwrap();
        assert_eq!(metadata(&db, id).await.1, Some(99));
    }
    let with_receipt = capture(&repo, select("extension-center", true, true)).await;
    // A matching archive may supply a receipt the target does not yet have.
    sqlx::query("update lifecycle_outbox set legacy_handoff_sequence=null where event_id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    repo.restore(reader(with_receipt), "key", "key", true)
        .await
        .unwrap();
    assert_eq!(metadata(&db, id).await.1, Some(99));
}

#[tokio::test]
async fn selective_publication_protocol_rejects_identity_body_version_and_receipt_collision() {
    let (db, _) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let id = source(&db, 1, Some(77)).await;
    let bytes = capture(&repo, select("extension-center", true, true)).await;
    let original: Value =
        sqlx::query_scalar("select to_jsonb(t) from lifecycle_outbox t where event_id=$1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
    for (field, value) in [
        ("native_publication_version", json!(2)),
        ("legacy_handoff_sequence", json!(78)),
        ("transaction_id", json!(Uuid::now_v7())),
        ("contract_id", json!("another.contract")),
        ("contract_version", json!("2")),
        ("canonical_payload", json!("\\x616c7465726564")),
        ("occurred_at", json!("2000-01-01T00:00:00+00:00")),
        ("created_at", json!("2000-01-01T00:00:00+00:00")),
    ] {
        let conflicting = rewrite(&bytes, |position, row| {
            if position > 0 && row["table"] == "lifecycle_outbox" {
                row["row"][field] = value.clone();
            }
        });
        denied(&repo, conflicting).await;
        let current: Value =
            sqlx::query_scalar("select to_jsonb(t) from lifecycle_outbox t where event_id=$1")
                .bind(id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(current, original, "collision changed {field}");
    }
    // The reverse conversion is forbidden too, including a genuinely old archive.
    sqlx::query("update lifecycle_outbox set native_publication_version=2 where event_id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    denied(&repo, historical(&bytes)).await;
    assert_eq!(metadata(&db, id).await.0, 2);
}

#[tokio::test]
async fn selective_publication_protocol_rejects_malformed_metadata_and_unrelated_schema_drift() {
    let (db, _) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let id = source(&db, 1, None).await;
    let bytes = capture(&repo, select("extension-center", true, true)).await;
    for (field, value) in [
        ("native_publication_version", Value::Null),
        ("native_publication_version", json!(0)),
        ("native_publication_version", json!(3)),
        ("native_publication_version", json!("1")),
        ("native_publication_version", json!(1.5)),
        ("legacy_handoff_sequence", json!(0)),
        ("legacy_handoff_sequence", json!(-1)),
        ("legacy_handoff_sequence", json!("99")),
    ] {
        denied(
            &repo,
            rewrite(&bytes, |position, row| {
                if position > 0 && row["table"] == "lifecycle_outbox" {
                    row["row"][field] = value.clone();
                }
            }),
        )
        .await;
    }
    denied(
        &repo,
        rewrite(&bytes, |position, row| {
            if position > 0 && row["table"] == "lifecycle_outbox" {
                let object = row["row"].as_object_mut().unwrap();
                object.remove("native_publication_version");
                object.remove("legacy_handoff_sequence");
            }
        }),
    )
    .await;
    for field in ["native_publication_version", "legacy_handoff_sequence"] {
        denied(
            &repo,
            rewrite(&bytes, |position, row| {
                if position > 0 && row["table"] == "lifecycle_outbox" {
                    row["row"].as_object_mut().unwrap().remove(field);
                }
            }),
        )
        .await;
        denied(
            &repo,
            rewrite(&bytes, |position, header| {
                if position == 0 {
                    for table in header["tables"].as_array_mut().unwrap() {
                        if table["name"] == "lifecycle_outbox" {
                            table["columns"]
                                .as_array_mut()
                                .unwrap()
                                .retain(|c| c["name"] != field);
                        }
                    }
                }
            }),
        )
        .await;
    }
    denied(
        &repo,
        rewrite(&historical(&bytes), |position, header| {
            if position == 0 {
                for table in header["tables"].as_array_mut().unwrap() {
                    if table["name"] == "lifecycle_outbox" {
                        table["columns"][0]["sql_type"] = json!("text");
                    }
                }
            }
        }),
    )
    .await;
    assert_eq!(
        metadata(&db, id).await,
        (1, None, b"original immutable fact".to_vec())
    );
}
