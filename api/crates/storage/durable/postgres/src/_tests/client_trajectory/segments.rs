use super::*;

async fn seed() -> (sqlx::PgPool, PgControlPlaneStore, Uuid, Uuid) {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let request = Uuid::now_v7();
    begin(&store, flow, None, request).await;
    (pool, store, flow, request)
}

fn input(request: Uuid, index: u32) -> AppendClientTrajectoryArchiveInput {
    AppendClientTrajectoryArchiveInput {
        request_id: request,
        part_id: Uuid::now_v7(),
        transport: ClientTrajectoryTransport::Http,
        frames: vec![ClientTrajectoryArchiveFrame {
            sequence: 0,
            kind: ClientTrajectoryFrameKind::ResponseSse,
            observed_at: format!("time\0 北京 {index}"),
            bytes: [index.to_be_bytes().to_vec(), vec![0, 255]].concat(),
        }],
    }
}

async fn terminal(pool: &sqlx::PgPool, request: Uuid) {
    sqlx::query("update client_trajectory_captures set status='complete' where request_id=$1")
        .bind(request)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn segment_publication_retry_mixed_cursors_and_rollback_preserve_all_originals() {
    let (pool, store, flow, request) = seed().await;
    let mut inputs = Vec::new();
    for index in 0..40 {
        let part = input(request, index);
        store.append_client_trajectory_archive(&part).await.unwrap();
        inputs.push(part);
    }
    let expected = store
        .read_client_trajectory_archive(request, 0, 128)
        .await
        .unwrap();
    terminal(&pool, request).await;
    let (a, b) = tokio::join!(
        store.seal_and_publish_client_archive(request),
        store.seal_and_publish_client_archive(request)
    );
    assert_eq!(a.unwrap() + b.unwrap(), 40);
    let counts:(i64,i64) = sqlx::query_as("select (select count(*) from client_trajectory_archive_parts where request_id=$1),(select count(*) from client_trajectory_archive_segments where request_id=$1)")
        .bind(request).fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (0, 1));
    let reopened = PgControlPlaneStore::new(pool.clone());
    for cursor in [0, 1, 7, 38, 39, 40] {
        let actual = reopened
            .read_client_trajectory_archive(request, cursor, 7)
            .await
            .unwrap();
        let wanted: Vec<_> = expected
            .iter()
            .filter(|frame| frame.sequence > cursor)
            .take(7)
            .cloned()
            .collect();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(wanted).unwrap()
        );
    }
    assert_eq!(
        reopened
            .append_client_trajectory_archive(&inputs[3])
            .await
            .unwrap()
            .persisted_through,
        40
    );
    let foreign_request = Uuid::now_v7();
    begin(&store, flow, None, foreign_request).await;
    let mut wrong = inputs[3].clone();
    wrong.request_id = foreign_request;
    assert!(store
        .append_client_trajectory_archive(&wrong)
        .await
        .is_err());
    let tail = input(request, 40);
    assert_eq!(
        store
            .append_client_trajectory_archive(&tail)
            .await
            .unwrap()
            .persisted_through,
        41
    );
    let mixed = store
        .read_client_trajectory_archive(request, 38, 7)
        .await
        .unwrap();
    assert_eq!(
        mixed.iter().map(|f| f.sequence).collect::<Vec<_>>(),
        vec![39, 40, 41]
    );
    let all = store
        .read_client_trajectory_archive(request, 0, 128)
        .await
        .unwrap();
    assert_eq!(
        store
            .restore_client_trajectory_archive_directory(flow, request)
            .await
            .unwrap(),
        40
    );
    assert_eq!(
        store
            .restore_client_trajectory_archive_directory(flow, request)
            .await
            .unwrap(),
        0
    );
    let restored = store
        .read_client_trajectory_archive(request, 0, 128)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(all).unwrap(),
        serde_json::to_value(restored).unwrap()
    );
    let identities:Vec<Uuid> = sqlx::query_scalar("select part_id from client_trajectory_archive_parts where request_id=$1 order by first_sequence")
        .bind(request).fetch_all(&pool).await.unwrap();
    assert_eq!(
        &identities[..40],
        inputs.iter().map(|p| p.part_id).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn segment_cleanup_failure_rolls_back_manifest_and_concurrent_readers_remain_complete() {
    let (pool, store, _, request) = seed().await;
    for index in 0..20 {
        store
            .append_client_trajectory_archive(&input(request, index))
            .await
            .unwrap();
    }
    terminal(&pool, request).await;
    store
        .seal_client_trajectory_archive_request(request)
        .await
        .unwrap();
    let expected = store
        .read_client_trajectory_archive(request, 0, 128)
        .await
        .unwrap();
    sqlx::raw_sql("create function refuse_retirement() returns trigger language plpgsql as $$ begin raise exception 'controlled cleanup failure'; end $$; create trigger refuse_retirement before delete on client_trajectory_archive_parts for each row execute function refuse_retirement()")
        .execute(&pool).await.unwrap();
    assert!(store
        .publish_client_archive_segments(request)
        .await
        .is_err());
    let counts:(i64,i64) = sqlx::query_as("select (select count(*) from client_trajectory_archive_parts where request_id=$1),(select count(*) from client_trajectory_archive_segments where request_id=$1)")
        .bind(request).fetch_one(&pool).await.unwrap();
    assert_eq!(
        counts,
        (20, 0),
        "publication and staging cleanup must be atomic"
    );
    sqlx::query("drop trigger refuse_retirement on client_trajectory_archive_parts")
        .execute(&pool)
        .await
        .unwrap();
    let read = async {
        for _ in 0..10 {
            let got = store
                .read_client_trajectory_archive(request, 0, 128)
                .await
                .unwrap();
            assert_eq!(
                serde_json::to_value(got).unwrap(),
                serde_json::to_value(&expected).unwrap()
            );
            tokio::task::yield_now().await;
        }
    };
    let (_, sealed) = tokio::join!(read, store.publish_client_archive_segments(request));
    assert_eq!(sealed.unwrap(), 20);
}
