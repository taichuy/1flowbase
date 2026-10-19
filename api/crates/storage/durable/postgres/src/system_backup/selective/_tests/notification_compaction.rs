use super::*;
use control_plane_contracts::ports::*;
use extension_contracts::{
    AfterCommitFact, LifecycleContract, LifecycleFactId, LifecycleTransactionId,
};
use serde_json::{json, Value};
use std::io::{Read, Write};
use time::{Duration, OffsetDateTime};

fn rewrite(bytes: &[u8], mut change: impl FnMut(usize, &mut Value) -> bool) -> Vec<u8> {
    let mut text = String::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_string(&mut text)
        .unwrap();
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    for (index, line) in text.lines().enumerate() {
        let mut row: Value = serde_json::from_str(line).unwrap();
        if change(index, &mut row) {
            writeln!(encoder, "{row}").unwrap();
        }
    }
    encoder.finish().unwrap()
}

fn without_digest(bytes: &[u8]) -> Vec<u8> {
    rewrite(bytes, |index, row| {
        if index == 0 {
            for table in row["tables"].as_array_mut().unwrap() {
                if table["name"] == "lifecycle_outbox" {
                    table["columns"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|c| c["name"] != "notification_payload_digest");
                }
            }
        } else if row["table"] == "lifecycle_outbox" {
            row["row"]
                .as_object_mut()
                .unwrap()
                .remove("notification_payload_digest");
        }
        true
    })
}

async fn notification() -> (
    PgSelectiveBackupRepository,
    crate::PgControlPlaneStore,
    PublishOptionalNotificationInput,
    Uuid,
) {
    let (db, _) = fixture().await;
    let catalog = LifecyclePublicationCatalog::new([(
        (
            CreditNotificationFact::CONTRACT_ID.into(),
            CreditNotificationFact::CONTRACT_VERSION.into(),
        ),
        LifecyclePublicationPlan {
            graph_fingerprint: "compaction-backup-frozen-v1".into(),
            subscribers: ["a", "b"]
                .into_iter()
                .map(|id| LifecycleSubscriberTarget {
                    subscriber_id: id.into(),
                    handler_id: format!("handler.{id}"),
                    handler_version: "v1".into(),
                })
                .collect(),
        },
    )])
    .unwrap();
    let store =
        crate::PgControlPlaneStore::new(db.clone()).with_lifecycle_publication_catalog(catalog);
    let event_id = Uuid::now_v7();
    let transaction_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let occurred_at = OffsetDateTime::now_utc();
    let fact = AfterCommitFact::new(
        LifecycleFactId::new(event_id.to_string()).unwrap(),
        LifecycleTransactionId::new(transaction_id.to_string()).unwrap(),
        (occurred_at.unix_timestamp_nanos() / 1_000_000) as i64,
        CreditNotificationFact {
            workspace_id,
            account_id: None,
            event_type: CreditNotificationEventType::CreditSettled,
            payload: json!({"original": "x".repeat(1024), "charged_amount":"2.50"}),
        },
    );
    let input = PublishOptionalNotificationInput {
        source_event_id: event_id,
        transaction_id,
        workspace_id: Some(workspace_id),
        occurred_at,
        contract: OptionalNotificationContract::Credit,
        canonical_payload: serde_json::to_vec(&fact).unwrap(),
    };
    let OptionalNotificationPublicationOutcome::Enqueued {
        event_id: publication,
        target_count: 2,
    } = store.publish_optional_notification(&input).await.unwrap()
    else {
        panic!("fixture requires two real targets")
    };
    (
        PgSelectiveBackupRepository::new(db),
        store,
        input,
        publication,
    )
}

async fn complete(store: &crate::PgControlPlaneStore) {
    let worker = Uuid::now_v7();
    let claims = store
        .claim_lifecycle_facts(worker, 10, Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(claims.len(), 2);
    for claim in claims {
        store
            .mark_lifecycle_fact_delivered(
                claim.event_id,
                &claim.subscriber_id,
                worker,
                claim.claim_id.unwrap(),
            )
            .await
            .unwrap();
    }
}

async fn snapshot(store: &crate::PgControlPlaneStore, id: Uuid) -> (Value, Vec<Value>) {
    let parent = sqlx::query_scalar("select to_jsonb(t) from lifecycle_outbox t where event_id=$1")
        .bind(id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let targets = sqlx::query_scalar("select to_jsonb(t) from lifecycle_outbox_deliveries t where event_id=$1 order by subscriber_id")
        .bind(id).fetch_all(store.pool()).await.unwrap();
    (parent, targets)
}

async fn denied(repo: &PgSelectiveBackupRepository, bytes: Vec<u8>) {
    if let Ok(preview) = repo.preflight(reader(bytes.clone()), "key", "key").await {
        assert!(
            !preview.failures.is_empty(),
            "invalid notification archive passed preflight"
        );
    }
    assert!(repo
        .restore(reader(bytes), "key", "key", true)
        .await
        .is_err());
}

#[tokio::test]
async fn selective_notification_compaction_restores_full_to_completed_compact_representation() {
    let (repo, store, _, id) = notification().await;
    let full = capture(&repo, select("extension-center", true, true)).await;
    complete(&store).await;
    let expected = snapshot(&store, id).await;
    assert_eq!(expected.0["canonical_payload"], "\\x");
    let compact = capture(&repo, select("extension-center", true, true)).await;
    sqlx::query("delete from lifecycle_outbox")
        .execute(store.pool())
        .await
        .unwrap();
    repo.restore(reader(full), "key", "key", true)
        .await
        .unwrap();
    repo.restore(reader(compact), "key", "key", true)
        .await
        .unwrap();
    assert_eq!(snapshot(&store, id).await, expected);
}

#[tokio::test]
async fn selective_notification_compaction_old_full_restore_and_replay_cannot_requeue_or_refill() {
    let (repo, store, input, id) = notification().await;
    let full = capture(&repo, select("extension-center", true, true)).await;
    complete(&store).await;
    let expected = snapshot(&store, id).await;
    let compact = capture(&repo, select("extension-center", true, true)).await;
    sqlx::query("delete from lifecycle_outbox")
        .execute(store.pool())
        .await
        .unwrap();
    repo.restore(reader(compact), "key", "key", true)
        .await
        .unwrap();
    for bytes in [full.clone(), without_digest(&full)] {
        repo.restore(reader(bytes), "key", "key", true)
            .await
            .unwrap();
        assert_eq!(snapshot(&store, id).await, expected);
        assert_eq!(
            store.publish_optional_notification(&input).await.unwrap(),
            OptionalNotificationPublicationOutcome::Enqueued {
                event_id: id,
                target_count: 2
            }
        );
        assert!(store
            .claim_lifecycle_facts(Uuid::now_v7(), 10, Duration::seconds(30))
            .await
            .unwrap()
            .is_empty());
    }
    assert_eq!(snapshot(&store, id).await, expected);
}

#[tokio::test]
async fn selective_notification_compaction_rejects_pending_empty_body_or_incomplete_target_manifest(
) {
    let (repo, store, _, id) = notification().await;
    let full = capture(&repo, select("extension-center", true, true)).await;
    denied(
        &repo,
        rewrite(&full, |index, row| {
            if index > 0 && row["table"] == "lifecycle_outbox" {
                row["row"]["canonical_payload"] = json!("\\x");
            }
            true
        }),
    )
    .await;
    complete(&store).await;
    let compact = capture(&repo, select("extension-center", true, true)).await;
    let expected = snapshot(&store, id).await;
    denied(
        &repo,
        rewrite(&compact, |index, row| {
            if index > 0 && row["table"] == "lifecycle_outbox_deliveries" {
                row["row"]["status"] = json!("pending");
                row["row"]["delivered_at"] = Value::Null;
            }
            true
        }),
    )
    .await;
    denied(
        &repo,
        rewrite(&compact, |index, row| {
            !(index > 0 && row["table"] == "lifecycle_outbox_deliveries")
        }),
    )
    .await;
    assert_eq!(snapshot(&store, id).await, expected);
    // The compact archive carries its own exact receipt manifest, including when
    // the destination has no prior target history to compare against.
    sqlx::query("delete from lifecycle_outbox")
        .execute(store.pool())
        .await
        .unwrap();
    denied(
        &repo,
        rewrite(&compact, |index, row| {
            !(index > 0
                && row["table"] == "lifecycle_outbox_deliveries"
                && row["row"]["subscriber_id"] == "b")
        }),
    )
    .await;
    denied(
        &repo,
        rewrite(&compact, |index, row| {
            if index > 0
                && row["table"] == "lifecycle_outbox_deliveries"
                && row["row"]["subscriber_id"] == "a"
            {
                row["row"]["handler_id"] = json!("retargeted");
            }
            true
        }),
    )
    .await;
    for change in ["missing", "count", "digest"] {
        denied(
            &repo,
            rewrite(&compact, |index, row| {
                if index > 0 && row["table"] == "lifecycle_outbox" {
                    match change {
                        "missing" => {
                            row.as_object_mut().unwrap().remove("notification_receipts");
                        }
                        "count" => row["notification_receipts"]["target_count"] = json!(1),
                        _ => {
                            row["notification_receipts"]["target_identity_digest"] =
                                json!("00".repeat(32))
                        }
                    }
                }
                true
            }),
        )
        .await;
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from lifecycle_outbox")
            .fetch_one(store.pool())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn selective_notification_compaction_rejects_wrong_digest_full_and_compact_or_original_fact()
{
    let (repo, store, _, id) = notification().await;
    let full = capture(&repo, select("extension-center", true, true)).await;
    for digest in [json!(format!("\\x{}", "00".repeat(32))), json!("\\x01")] {
        denied(
            &repo,
            rewrite(&full, |index, row| {
                if index > 0 && row["table"] == "lifecycle_outbox" {
                    row["row"]["notification_payload_digest"] = digest.clone();
                }
                true
            }),
        )
        .await;
    }
    complete(&store).await;
    let compact = capture(&repo, select("extension-center", true, true)).await;
    for (field, value) in [
        ("notification_payload_digest", Value::Null),
        (
            "notification_payload_digest",
            json!(format!("\\x{}", "00".repeat(32))),
        ),
        ("contract_id", json!("model_definition.committed")),
        ("contract_version", json!("v2")),
    ] {
        denied(
            &repo,
            rewrite(&compact, |index, row| {
                if index > 0 && row["table"] == "lifecycle_outbox" {
                    row["row"][field] = value.clone();
                }
                true
            }),
        )
        .await;
    }
    assert_eq!(snapshot(&store, id).await.0["canonical_payload"], "\\x");
}

#[tokio::test]
async fn selective_notification_compaction_digest_does_not_override_identity_graph_or_targets() {
    let (repo, store, _, id) = notification().await;
    complete(&store).await;
    let compact = capture(&repo, select("extension-center", true, true)).await;
    let expected = snapshot(&store, id).await;
    for (field, value) in [
        ("transaction_id", json!(Uuid::now_v7())),
        ("occurred_at", json!("2000-01-01T00:00:00+00:00")),
        ("graph_fingerprint", json!("new-graph")),
    ] {
        denied(
            &repo,
            rewrite(&compact, |index, row| {
                if index > 0 && row["table"] == "lifecycle_outbox" {
                    row["row"][field] = value.clone();
                }
                true
            }),
        )
        .await;
    }
    for field in ["handler_id", "handler_version", "subscriber_id"] {
        denied(
            &repo,
            rewrite(&compact, |index, row| {
                if index > 0
                    && row["table"] == "lifecycle_outbox_deliveries"
                    && row["row"]["subscriber_id"] == "a"
                {
                    row["row"][field] = json!("replacement");
                }
                true
            }),
        )
        .await;
    }
    // A nonempty subset of the frozen receipt set is still incomplete.
    denied(
        &repo,
        rewrite(&compact, |index, row| {
            !(index > 0
                && row["table"] == "lifecycle_outbox_deliveries"
                && row["row"]["subscriber_id"] == "b")
        }),
    )
    .await;
    assert_eq!(snapshot(&store, id).await, expected);
}

#[tokio::test]
async fn selective_notification_compaction_supports_exact_intermediate_header_without_digest() {
    let (repo, store, _, id) = notification().await;
    let full = without_digest(&capture(&repo, select("extension-center", true, true)).await);
    sqlx::query("delete from lifecycle_outbox")
        .execute(store.pool())
        .await
        .unwrap();
    repo.restore(reader(full.clone()), "key", "key", true)
        .await
        .unwrap();
    let row = snapshot(&store, id).await.0;
    assert!(row["notification_payload_digest"].is_null());
    assert_ne!(row["canonical_payload"], "\\x");
    // Current headers cannot silently omit digest values from rows.
    let current = capture(&repo, select("extension-center", true, true)).await;
    denied(
        &repo,
        rewrite(&current, |index, row| {
            if index > 0 && row["table"] == "lifecycle_outbox" {
                row["row"]
                    .as_object_mut()
                    .unwrap()
                    .remove("notification_payload_digest");
            }
            true
        }),
    )
    .await;
}

#[tokio::test]
async fn current_backup_roundtrip_excludes_retired_ledger_and_preserves_model_source_and_receipts()
{
    let (repo, store, _, typed_event) = notification().await;
    let source = Uuid::now_v7();
    let transaction = Uuid::now_v7();
    let now = OffsetDateTime::now_utc();
    let original = AfterCommitFact::new(
        LifecycleFactId::new(source.to_string()).unwrap(),
        LifecycleTransactionId::new(transaction.to_string()).unwrap(),
        (now.unix_timestamp_nanos() / 1_000_000) as i64,
        ModelDefinitionCommittedFact {
            model_definition_id: Uuid::now_v7(),
            scope_kind: domain::DataModelScopeKind::Workspace,
            scope_id: Uuid::now_v7(),
        },
    );
    let original_body = serde_json::to_vec(&original).unwrap();
    sqlx::query("insert into lifecycle_outbox(event_id,transaction_id,contract_id,contract_version,canonical_payload,occurred_at,status,delivered_at,graph_fingerprint) values($1,$2,'model_definition.committed','v1',$3,$4,'delivered',$4,'original-model-snapshot')")
        .bind(source).bind(transaction).bind(&original_body).bind(now).execute(store.pool()).await.unwrap();
    sqlx::query("insert into lifecycle_outbox_deliveries(event_id,subscriber_id,handler_id,handler_version,status,delivered_at) values($1,'source-owner','source-owner.handler','v1','delivered',now())")
        .bind(source).execute(store.pool()).await.unwrap();
    let source_before = snapshot(&store, source).await;
    let typed_before = snapshot(&store, typed_event).await;
    let mut selection = select("extension-center", true, true);
    selection.extend(select("system-runtime", true, true));
    let bytes = capture(&repo, selection).await;
    let mut text = String::new();
    flate2::read::GzDecoder::new(bytes.as_slice())
        .read_to_string(&mut text)
        .unwrap();
    let header: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert!(!header["tables"]
        .as_array()
        .unwrap()
        .iter()
        .any(|table| table["name"] == "legacy_event_recovery_ledger"));
    let lifecycle = header["tables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|table| table["name"] == "lifecycle_outbox")
        .unwrap();
    assert!(!lifecycle["columns"]
        .as_array()
        .unwrap()
        .iter()
        .any(|column| column["name"] == "native_publication_version"
            || column["name"] == "legacy_handoff_sequence"));
    let preview = repo
        .preflight(reader(bytes.clone()), "key", "key")
        .await
        .unwrap();
    assert!(preview.failures.is_empty(), "{:?}", preview.failures);
    sqlx::query("delete from lifecycle_outbox")
        .execute(store.pool())
        .await
        .unwrap();
    repo.restore(reader(bytes), "key", "key", true)
        .await
        .unwrap();
    assert_eq!(snapshot(&store, source).await, source_before);
    assert_eq!(snapshot(&store, typed_event).await, typed_before);
    let restored_body: Vec<u8> =
        sqlx::query_scalar("select canonical_payload from lifecycle_outbox where event_id=$1")
            .bind(source)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(restored_body, original_body);
}
