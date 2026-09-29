use super::*;
use sha2::{Digest, Sha256};

fn locator_range(bytes: &[u8]) -> (usize, usize) {
    assert_eq!(&bytes[..4], b"CAL1");
    let mut input = &bytes[4..];
    let mut take = || {
        let mut n = 0usize;
        for shift in (0..70).step_by(7) {
            let b = input[0];
            input = &input[1..];
            n |= usize::from(b & 127) << shift;
            if b & 128 == 0 {
                return n;
            }
        }
        panic!("invalid test locator");
    };
    let offset = take();
    let length = take();
    (offset, length)
}

async fn old_block(store: &PgControlPlaneStore, request: Uuid) {
    // Exact prior v2/v1 physical layout, derived from authenticated final reader
    // output. This fixture exercises the real incremental maintenance entry.
    let (id, bytes): (Uuid, Vec<u8>) = sqlx::query_as(
        "select block_id,bytes from client_trajectory_archive_blocks where request_id=$1",
    )
    .bind(request)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let raw = zstd::stream::decode_all(bytes.as_slice()).unwrap();
    let rows: Vec<(Uuid,Vec<u8>,i64,i64)> = sqlx::query_as("select part_id,frame_directory,block_offset,raw_byte_length from client_trajectory_archive_parts where block_id=$1 order by block_offset")
        .bind(id).fetch_all(store.pool()).await.unwrap();
    let mut end = 0usize;
    let mut tx = store.pool().begin().await.unwrap();
    for (part, locator, offset, length) in rows {
        assert_eq!(offset as usize, end);
        end += length as usize;
        let (start, n) = locator_range(&locator);
        sqlx::query("update client_trajectory_archive_parts set codec_version=2,frame_directory=$2 where part_id=$1")
            .bind(part).bind(&raw[start..start+n]).execute(&mut *tx).await.unwrap();
    }
    let body = &raw[..end];
    sqlx::query("update client_trajectory_archive_blocks set codec_version=1,bytes=$2,raw_byte_length=$3,raw_checksum=$4 where block_id=$1")
        .bind(id).bind(zstd::stream::encode_all(body,3).unwrap()).bind(end as i64).bind(Sha256::digest(body).to_vec()).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn packed_directories_upgrade_prior_blocks_atomically_with_gaps_and_original_ids() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let request = Uuid::now_v7();
    begin(&store, flow, None, request).await;
    let input = |part_id, bytes: &[u8]| AppendClientTrajectoryArchiveInput {
        request_id: request,
        part_id,
        transport: ClientTrajectoryTransport::Http,
        frames: vec![ClientTrajectoryArchiveFrame {
            sequence: 0,
            kind: ClientTrajectoryFrameKind::ResponseSse,
            observed_at: "exact\0time🌍".into(),
            bytes: bytes.to_vec(),
        }],
    };
    let first = input(Uuid::now_v7(), b"first\0\xff");
    store
        .append_client_trajectory_archive(&first)
        .await
        .unwrap();
    let legacy = Uuid::now_v7();
    sqlx::query("insert into client_trajectory_archive_parts(part_id,request_id,first_sequence,last_sequence,frames,bytes) values($1,$2,2,2,$3,$4)")
        .bind(legacy).bind(request).bind(json!([{"sequence":2,"kind":"response_sse","observed_at":AT,"offset":0,"length":6,"format":"wire"}])).bind(b"legacy".as_slice()).execute(&pool).await.unwrap();
    sqlx::query(
        "update client_trajectory_archive_heads set persisted_through=2 where request_id=$1",
    )
    .bind(request)
    .execute(&pool)
    .await
    .unwrap();
    store
        .append_client_trajectory_archive(&input(Uuid::now_v7(), b"last"))
        .await
        .unwrap();
    sqlx::query("update client_trajectory_captures set status='complete' where request_id=$1")
        .bind(request)
        .execute(&pool)
        .await
        .unwrap();
    let expected = store
        .read_client_trajectory_archive(request, 0, 128)
        .await
        .unwrap();
    assert_eq!(
        store
            .seal_client_trajectory_archive_request(request)
            .await
            .unwrap(),
        2
    );
    old_block(&store, request).await;
    let before: Vec<(Uuid, i64, i64, i16, Option<Vec<u8>>, Option<Vec<u8>>)> = sqlx::query_as("select part_id,first_sequence,last_sequence,codec_version,frame_directory,raw_checksum from client_trajectory_archive_parts where request_id=$1 order by first_sequence")
        .bind(request).fetch_all(&pool).await.unwrap();
    sqlx::raw_sql("create function packed_fixture_failure() returns trigger language plpgsql as $$ begin raise exception 'controlled packed publication failure'; end $$;create trigger packed_fixture_failure before update on client_trajectory_archive_parts for each row when(new.codec_version=3 and old.first_sequence=3) execute function packed_fixture_failure();").execute(&pool).await.unwrap();
    let failure = store
        .seal_client_trajectory_archive_request(request)
        .await
        .unwrap_err();
    assert!(format!("{failure:#}").contains("controlled packed publication failure"));
    let after: Vec<(Uuid, i64, i64, i16, Option<Vec<u8>>, Option<Vec<u8>>)> = sqlx::query_as("select part_id,first_sequence,last_sequence,codec_version,frame_directory,raw_checksum from client_trajectory_archive_parts where request_id=$1 order by first_sequence")
        .bind(request).fetch_all(&pool).await.unwrap();
    assert_eq!(before, after);
    let block_version: i16 = sqlx::query_scalar(
        "select codec_version from client_trajectory_archive_blocks where request_id=$1",
    )
    .bind(request)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(block_version, 1);
    sqlx::query("drop trigger packed_fixture_failure on client_trajectory_archive_parts")
        .execute(&pool)
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        store.seal_client_trajectory_archive_request(request),
        store.seal_client_trajectory_archive_request(request)
    );
    assert_eq!(a.unwrap() + b.unwrap(), 2);
    assert_eq!(
        store
            .seal_client_trajectory_archive_request(request)
            .await
            .unwrap(),
        0
    );
    for cursor in 0..3 {
        let page = store
            .read_client_trajectory_archive(request, cursor, 1)
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(page).unwrap(),
            serde_json::to_value(&expected[cursor as usize..cursor as usize + 1]).unwrap()
        );
    }
    assert_eq!(
        store
            .append_client_trajectory_archive(&first)
            .await
            .unwrap()
            .persisted_through,
        3
    );
    let stored: Vec<u8> = sqlx::query_scalar(
        "select frame_directory from client_trajectory_archive_parts where part_id=$1",
    )
    .bind(first.part_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    for corrupted in [
        stored[..stored.len() - 1].to_vec(),
        [stored.clone(), vec![0]].concat(),
        b"CAL1\x80\x00\x01".to_vec(),
        b"CAL1\x7f\x7f".to_vec(),
    ] {
        sqlx::query(
            "update client_trajectory_archive_parts set frame_directory=$2 where part_id=$1",
        )
        .bind(first.part_id)
        .bind(corrupted)
        .execute(&pool)
        .await
        .unwrap();
        assert!(store
            .read_client_trajectory_archive(request, 0, 1)
            .await
            .is_err());
    }
    sqlx::query("update client_trajectory_archive_parts set frame_directory=$2 where part_id=$1")
        .bind(first.part_id)
        .bind(stored)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(
            store
                .read_client_trajectory_archive(request, 0, 128)
                .await
                .unwrap()
        )
        .unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}
