//! Formal-migration PostgreSQL fixtures for the centralized assembly Test Batch.
use super::*;

fn raw_frame(bytes: Vec<u8>) -> ClientTrajectoryArchiveFrame {
    ClientTrajectoryArchiveFrame {
        sequence: 0,
        kind: ClientTrajectoryFrameKind::ResponseSse,
        observed_at: " unusual timestamp \0 北京 🌍 \n".into(),
        bytes,
    }
}

async fn bound(store: &PgControlPlaneStore, flow: Uuid, request: Uuid) {
    begin(store, flow, None, request).await;
    append(
        store,
        flow,
        None,
        request,
        ClientTrajectoryFact::Step {
            step: Box::new(step(flow, None, request, request, "submitted", "request")),
        },
    )
    .await;
}

async fn terminal(store: &PgControlPlaneStore, request: Uuid) {
    // Fixture marks the already-bound capture terminal directly so the explicit
    // maintenance assertions cannot race the production post-commit scheduler.
    sqlx::query("update client_trajectory_captures set status='complete' where request_id=$1")
        .bind(request)
        .execute(store.pool())
        .await
        .unwrap();
}

async fn part(
    store: &PgControlPlaneStore,
    request: Uuid,
    bytes: Vec<u8>,
) -> AppendClientTrajectoryArchiveInput {
    let input = AppendClientTrajectoryArchiveInput {
        request_id: request,
        part_id: Uuid::now_v7(),
        transport: ClientTrajectoryTransport::Http,
        frames: vec![raw_frame(bytes)],
    };
    store
        .append_client_trajectory_archive(&input)
        .await
        .unwrap();
    input
}

fn exact(actual: &[ClientTrajectoryArchiveFrame], expected: &[ClientTrajectoryArchiveFrame]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.sequence, expected.sequence);
        assert_eq!(actual.kind, expected.kind);
        assert_eq!(actual.observed_at, expected.observed_at);
        assert_eq!(actual.bytes, expected.bytes);
    }
}

#[tokio::test]
async fn raw_blocks_terminal_only_concurrent_reentry_duplicate_oversize_and_cascade() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let request = Uuid::now_v7();
    bound(&store, flow, request).await;
    let first = part(
        &store,
        request,
        b"SSE split\0\xff data: {\"n\":18446744073709551617}\n".to_vec(),
    )
    .await;
    part(&store, request, Vec::new()).await;
    part(&store, request, vec![b'x'; 300 * 1024]).await;
    part(&store, request, b"\ndata: tail\n\n".to_vec()).await;
    assert_eq!(
        store
            .seal_client_trajectory_archive_request(request)
            .await
            .unwrap(),
        0
    );
    let expected = store
        .read_client_trajectory_archive(request, 0, 128)
        .await
        .unwrap();
    let anchors: Vec<(Uuid, i64, i64, i64, Vec<u8>)> = sqlx::query_as("select part_id,first_sequence,last_sequence,raw_byte_length,raw_checksum from client_trajectory_archive_parts where request_id=$1 order by first_sequence")
        .bind(request).fetch_all(&pool).await.unwrap();
    terminal(&store, request).await;
    let reopened = PgControlPlaneStore::new(pool.clone());
    let (a, b) = tokio::join!(
        store.seal_client_trajectory_archive_request(request),
        reopened.seal_client_trajectory_archive_request(request)
    );
    assert_eq!(
        a.unwrap() + b.unwrap(),
        4,
        "concurrent owners must switch each anchor exactly once"
    );
    assert_eq!(
        reopened
            .seal_client_trajectory_archive_request(request)
            .await
            .unwrap(),
        0
    );
    let after: Vec<(Uuid, i64, i64, i64, Vec<u8>)> = sqlx::query_as("select part_id,first_sequence,last_sequence,raw_byte_length,raw_checksum from client_trajectory_archive_parts where request_id=$1 order by first_sequence")
        .bind(request).fetch_all(&pool).await.unwrap();
    assert_eq!(
        anchors, after,
        "all original logical anchor metadata remains exact"
    );
    let sizes: Vec<i64> = sqlx::query_scalar("select raw_byte_length from client_trajectory_archive_blocks where request_id=$1 order by raw_byte_length")
        .bind(request).fetch_all(&pool).await.unwrap();
    assert_eq!(
        sizes.len(),
        3,
        "oversized original stands alone between bounded neighbors"
    );
    assert!(
        *sizes.last().unwrap() > 300 * 1024,
        "block also includes authenticated CAD1 directories"
    );
    let compact: (i64, i64) = sqlx::query_as("select count(*),sum(octet_length(bytes))::bigint from client_trajectory_archive_parts where request_id=$1 and codec_version=3 and block_id is not null and block_offset>=0")
        .bind(request).fetch_one(&pool).await.unwrap();
    assert_eq!(compact, (4, 0));
    exact(
        &reopened
            .read_client_trajectory_archive(request, 0, 128)
            .await
            .unwrap(),
        &expected,
    );
    for cursor in 0..4 {
        exact(
            &reopened
                .read_client_trajectory_archive(request, cursor, 1)
                .await
                .unwrap(),
            &expected[cursor as usize..cursor as usize + 1],
        );
    }
    let receipt = reopened
        .append_client_trajectory_archive(&first)
        .await
        .unwrap();
    assert_eq!(receipt.persisted_through, 4);
    assert_eq!(
        reopened
            .seal_client_trajectory_archive_request(request)
            .await
            .unwrap(),
        0
    );
    let mut foreign = first.clone();
    foreign.request_id = Uuid::now_v7();
    assert!(reopened
        .append_client_trajectory_archive(&foreign)
        .await
        .is_err());
    part(&reopened, request, b"late inline after EOF".to_vec()).await;
    assert_eq!(
        reopened
            .seal_client_trajectory_archive_request(request)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        reopened
            .read_client_trajectory_archive(request, 4, 1)
            .await
            .unwrap()[0]
            .bytes,
        b"late inline after EOF"
    );
    sqlx::query("delete from flow_runs where id=$1")
        .bind(flow)
        .execute(&pool)
        .await
        .unwrap();
    let counts: (i64,i64,i64) = sqlx::query_as("select (select count(*) from client_trajectory_archive_heads where request_id=$1),(select count(*) from client_trajectory_archive_parts where request_id=$1),(select count(*) from client_trajectory_archive_blocks where request_id=$1)")
        .bind(request).fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (0, 0, 0));
}

#[tokio::test]
async fn raw_blocks_atomic_publication_faults_preserve_inline_originals_and_receipt() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let request = Uuid::now_v7();
    bound(&store, flow, request).await;
    let input = part(&store, request, b"first original\0\xff".to_vec()).await;
    part(&store, request, b"second original".to_vec()).await;
    terminal(&store, request).await;
    let expected = store
        .read_client_trajectory_archive(request, 0, 128)
        .await
        .unwrap();
    sqlx::raw_sql("create function fixture_fail_block() returns trigger language plpgsql as $$ begin raise exception 'controlled archive publication failure'; end $$;")
        .execute(&pool).await.unwrap();
    for trigger in [
        "create trigger fixture_block_fault before insert on client_trajectory_archive_blocks for each row execute function fixture_fail_block()",
        "create trigger fixture_block_fault after insert on client_trajectory_archive_blocks for each row execute function fixture_fail_block()",
        "create trigger fixture_block_fault before update on client_trajectory_archive_parts for each row when (new.codec_version=3 and old.first_sequence=2) execute function fixture_fail_block()",
    ] {
        sqlx::raw_sql(trigger).execute(&pool).await.unwrap();
        let error = store.seal_client_trajectory_archive_request(request).await.err().unwrap();
        assert!(format!("{error:#}").contains("controlled archive publication failure"));
        exact(&store.read_client_trajectory_archive(request, 0, 128).await.unwrap(), &expected);
        let counts: (i64,i64) = sqlx::query_as("select (select count(*) from client_trajectory_archive_blocks where request_id=$1),(select count(*) from client_trajectory_archive_parts where request_id=$1 and codec_version=1 and block_id is null)")
            .bind(request).fetch_one(&pool).await.unwrap();
        assert_eq!(counts, (0,2), "rollback must remove inserted block and partial locator switches");
        assert_eq!(store.append_client_trajectory_archive(&input).await.unwrap().persisted_through, 2);
        let table = if trigger.contains("on client_trajectory_archive_blocks") { "client_trajectory_archive_blocks" } else { "client_trajectory_archive_parts" };
        sqlx::raw_sql(&format!("drop trigger fixture_block_fault on {table}")).execute(&pool).await.unwrap();
    }
    assert_eq!(
        store
            .seal_client_trajectory_archive_request(request)
            .await
            .unwrap(),
        2
    );
    exact(
        &store
            .read_client_trajectory_archive(request, 0, 128)
            .await
            .unwrap(),
        &expected,
    );
}

#[tokio::test]
async fn raw_blocks_strict_corruption_scope_and_snapshot_publication() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let request = Uuid::now_v7();
    bound(&store, flow, request).await;
    let first = part(&store, request, b"first frame\0\xff".to_vec()).await;
    part(&store, request, b"second frame".to_vec()).await;
    terminal(&store, request).await;
    let expected = store
        .read_client_trajectory_archive(request, 0, 128)
        .await
        .unwrap();
    // An older reader snapshot retains its inline body through publication;
    // no separate locator/body statement is used by production.
    let mut snapshot = pool.begin().await.unwrap();
    sqlx::query("set transaction isolation level repeatable read")
        .execute(&mut *snapshot)
        .await
        .unwrap();
    let old: (i16, Vec<u8>) = sqlx::query_as(
        "select codec_version,bytes from client_trajectory_archive_parts where part_id=$1",
    )
    .bind(first.part_id)
    .fetch_one(&mut *snapshot)
    .await
    .unwrap();
    assert_eq!(
        store
            .seal_client_trajectory_archive_request(request)
            .await
            .unwrap(),
        2
    );
    let old_after: (i16, Vec<u8>) = sqlx::query_as(
        "select codec_version,bytes from client_trajectory_archive_parts where part_id=$1",
    )
    .bind(first.part_id)
    .fetch_one(&mut *snapshot)
    .await
    .unwrap();
    assert_eq!(old, old_after);
    snapshot.commit().await.unwrap();
    let original: (Uuid,Vec<u8>,i64,Vec<u8>) = sqlx::query_as("select block_id,bytes,raw_byte_length,raw_checksum from client_trajectory_archive_blocks where request_id=$1")
        .bind(request).fetch_one(&pool).await.unwrap();
    for mutation in [
        "update client_trajectory_archive_blocks set bytes=substring(bytes from 1 for octet_length(bytes)-1) where block_id=$1",
        "update client_trajectory_archive_blocks set bytes=bytes || decode('00','hex') where block_id=$1",
        "update client_trajectory_archive_blocks set raw_byte_length=raw_byte_length+1 where block_id=$1",
        "update client_trajectory_archive_blocks set raw_checksum=decode(repeat('00',32),'hex') where block_id=$1",
    ] {
        sqlx::query(mutation).bind(original.0).execute(&pool).await.unwrap();
        assert!(store.read_client_trajectory_archive(request, 0, 1).await.is_err(), "whole block must be checked for one-frame page: {mutation}");
        sqlx::query("update client_trajectory_archive_blocks set bytes=$2,raw_byte_length=$3,raw_checksum=$4 where block_id=$1")
            .bind(original.0).bind(&original.1).bind(original.2).bind(&original.3).execute(&pool).await.unwrap();
    }
    let checksum: Vec<u8> = sqlx::query_scalar(
        "select raw_checksum from client_trajectory_archive_parts where part_id=$1",
    )
    .bind(first.part_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("update client_trajectory_archive_parts set raw_checksum=decode(repeat('00',32),'hex') where part_id=$1")
        .bind(first.part_id).execute(&pool).await.unwrap();
    assert!(
        store
            .read_client_trajectory_archive(request, 0, 1)
            .await
            .is_err(),
        "anchor CAD1 checksum is independently required"
    );
    sqlx::query("update client_trajectory_archive_parts set raw_checksum=$2 where part_id=$1")
        .bind(first.part_id)
        .bind(checksum)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("update client_trajectory_archive_parts set block_offset=999999 where part_id=$1")
        .bind(first.part_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(store
        .read_client_trajectory_archive(request, 0, 1)
        .await
        .is_err());
    sqlx::query("update client_trajectory_archive_parts set block_offset=0 where part_id=$1")
        .bind(first.part_id)
        .execute(&pool)
        .await
        .unwrap();
    let foreign = Uuid::now_v7();
    bound(&store, flow, foreign).await;
    let foreign_input = part(&store, foreign, b"foreign body".to_vec()).await;
    terminal(&store, foreign).await;
    store
        .seal_client_trajectory_archive_request(foreign)
        .await
        .unwrap();
    let foreign_block: Uuid =
        sqlx::query_scalar("select block_id from client_trajectory_archive_parts where part_id=$1")
            .bind(foreign_input.part_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        sqlx::query("update client_trajectory_archive_parts set block_id=$2 where part_id=$1")
            .bind(first.part_id)
            .bind(foreign_block)
            .execute(&pool)
            .await
            .is_err(),
        "composite FK rejects cross-request block"
    );
    exact(
        &store
            .read_client_trajectory_archive(request, 0, 128)
            .await
            .unwrap(),
        &expected,
    );
}

#[tokio::test]
async fn raw_blocks_mixed_legacy_inline_pages_and_corrupt_source_rejection() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let request = Uuid::now_v7();
    bound(&store, flow, request).await;
    part(&store, request, b"first\0\xff".to_vec()).await;
    part(&store, request, b"second".to_vec()).await;
    let legacy = json!({"direction":"emitted","encoding":"utf8","body":"legacy\0🌍","frame_kind":"response_json","kept":{"unknown":"exact"}});
    let legacy_bytes = serde_json::to_vec(&legacy).unwrap();
    let legacy_id = Uuid::now_v7();
    let directory = json!([{"sequence":3,"kind":"response_json","observed_at":AT,"offset":0,"length":legacy_bytes.len(),"format":"legacy_json"}]);
    sqlx::query("insert into client_trajectory_archive_parts(part_id,request_id,first_sequence,last_sequence,frames,bytes) values($1,$2,3,3,$3,$4)")
        .bind(legacy_id).bind(request).bind(&directory).bind(&legacy_bytes).execute(&pool).await.unwrap();
    sqlx::query(
        "update client_trajectory_archive_heads set persisted_through=3 where request_id=$1",
    )
    .bind(request)
    .execute(&pool)
    .await
    .unwrap();
    terminal(&store, request).await;
    assert_eq!(
        store
            .seal_client_trajectory_archive_request(request)
            .await
            .unwrap(),
        2
    );
    let inline = part(&store, request, b"still inline".to_vec()).await;
    let versions: Vec<i16> = sqlx::query_scalar("select codec_version from client_trajectory_archive_parts where request_id=$1 order by first_sequence")
        .bind(request).fetch_all(&pool).await.unwrap();
    assert_eq!(versions, [3, 3, 0, 1]);
    let page = store
        .client_trajectory_section(flow, None, request, "raw", Some(1), 2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page.items[0].sequence, 2);
    assert_eq!(page.items[0].value["body"], "second");
    assert_eq!(page.items[1].sequence, 3);
    assert_eq!(
        page.items[1].value, legacy,
        "legacy unknown fields and NUL survive mixed block pages"
    );
    assert_eq!(page.next_cursor, Some(3));
    let last = store
        .client_trajectory_section(flow, None, request, "raw", page.next_cursor, 2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(last.items[0].value["body"], "still inline");
    assert_eq!(last.next_cursor, None);
    let retained: (Value, Vec<u8>, i16) = sqlx::query_as(
        "select frames,bytes,codec_version from client_trajectory_archive_parts where part_id=$1",
    )
    .bind(legacy_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(retained, (directory, legacy_bytes, 0));
    let original_directory: Vec<u8> = sqlx::query_scalar(
        "select frame_directory from client_trajectory_archive_parts where part_id=$1",
    )
    .bind(inline.part_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("update client_trajectory_archive_parts set frame_directory=substring(frame_directory from 1 for octet_length(frame_directory)-1) where part_id=$1")
        .bind(inline.part_id).execute(&pool).await.unwrap();
    assert!(store
        .seal_client_trajectory_archive_request(request)
        .await
        .is_err());
    let intact: (i16,Option<Uuid>,i64) = sqlx::query_as("select codec_version,block_id,octet_length(bytes)::bigint from client_trajectory_archive_parts where part_id=$1")
        .bind(inline.part_id).fetch_one(&pool).await.unwrap();
    assert_eq!(intact.0, 1);
    assert_eq!(intact.1, None);
    assert!(intact.2 > 0);
    sqlx::query("update client_trajectory_archive_parts set frame_directory=$2 where part_id=$1")
        .bind(inline.part_id)
        .bind(original_directory)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        store
            .seal_client_trajectory_archive_request(request)
            .await
            .unwrap(),
        1
    );
    // Schema version validation and decoder version validation are separate
    // invariants. Deliberately bypass only the CHECK for the controlled negative.
    sqlx::query("alter table client_trajectory_archive_blocks drop constraint client_trajectory_archive_blocks_codec_version_check")
        .execute(&pool).await.unwrap();
    sqlx::query("update client_trajectory_archive_blocks set codec_version=99 where request_id=$1")
        .bind(request)
        .execute(&pool)
        .await
        .unwrap();
    assert!(store
        .read_client_trajectory_archive(request, 0, 1)
        .await
        .is_err());
    sqlx::query("update client_trajectory_archive_blocks set codec_version=2 where request_id=$1")
        .bind(request)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "alter table client_trajectory_archive_parts drop constraint client_archive_codec_columns",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("update client_trajectory_archive_parts set codec_version=99 where part_id=$1")
        .bind(inline.part_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(store
        .seal_client_trajectory_archive_request(request)
        .await
        .is_err());
    assert!(store
        .read_client_trajectory_archive(request, 3, 1)
        .await
        .is_err());
}
