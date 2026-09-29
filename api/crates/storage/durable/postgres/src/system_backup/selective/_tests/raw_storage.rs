use super::*;
use control_plane_contracts::ports::*;

#[tokio::test]
async fn selective_application_backup_restores_sealed_raw_blocks_and_original_part_receipts() {
    let (db, run) = crate::_tests::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = crate::PgControlPlaneStore::new(db.clone());
    let request = Uuid::now_v7();
    let integrity = |status: &str| AppendClientTrajectoryInput {
        flow_run_id: run,
        node_run_id: None,
        request_id: request,
        observed_at: "original\0timestamp".into(),
        fact: ClientTrajectoryFact::Integrity {
            status: status.into(),
            dropped_count: 0,
            persist_failed_count: 0,
        },
    };
    store
        .append_client_trajectory(&integrity("pending"))
        .await
        .unwrap();
    let input = AppendClientTrajectoryArchiveInput {
        request_id: request,
        part_id: Uuid::now_v7(),
        transport: ClientTrajectoryTransport::Http,
        frames: vec![ClientTrajectoryArchiveFrame {
            sequence: 0,
            kind: ClientTrajectoryFrameKind::ResponseSse,
            observed_at: "exact timestamp".into(),
            bytes: b"data: {\"n\":9007199254740993}\n\n\0\xff".to_vec(),
        }],
    };
    let receipt = store
        .append_client_trajectory_archive(&input)
        .await
        .unwrap();
    store
        .append_client_trajectory(&integrity("complete"))
        .await
        .unwrap();
    store
        .seal_client_trajectory_archive_request(request)
        .await
        .unwrap();
    let before = store
        .read_client_trajectory_archive(request, 0, 128)
        .await
        .unwrap();
    let version: i16 = sqlx::query_scalar(
        "select codec_version from client_trajectory_archive_parts where part_id=$1",
    )
    .bind(input.part_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(version, 2);
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let bytes = capture(&repo, select("applications", true, true)).await;
    repo.restore(reader(bytes.clone()), "key", "key", true)
        .await
        .unwrap();
    sqlx::query("delete from client_trajectory_archive_heads where request_id=$1")
        .bind(request)
        .execute(&db)
        .await
        .unwrap();
    let empty: (i64,i64) = sqlx::query_as("select (select count(*) from client_trajectory_archive_parts),(select count(*) from client_trajectory_archive_blocks)").fetch_one(&db).await.unwrap();
    assert_eq!(empty, (0, 0));
    let preview = repo
        .preflight(reader(bytes.clone()), "key", "key")
        .await
        .unwrap();
    assert!(preview.failures.is_empty(), "{:?}", preview.failures);
    repo.restore(reader(bytes), "key", "key", true)
        .await
        .unwrap();
    let after = store
        .read_client_trajectory_archive(request, 0, 128)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
    assert_eq!(
        store
            .append_client_trajectory_archive(&input)
            .await
            .unwrap()
            .persisted_through,
        receipt.persisted_through
    );
}
