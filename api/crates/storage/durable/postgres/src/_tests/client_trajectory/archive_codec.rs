//! Real PostgreSQL fixtures run only with the assembled central Test Batch.
use super::*;

async fn bind_request(store: &PgControlPlaneStore, flow: Uuid, request: Uuid) {
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

fn frame(
    sequence: i64,
    kind: ClientTrajectoryFrameKind,
    time: &str,
    bytes: &[u8],
) -> ClientTrajectoryArchiveFrame {
    ClientTrajectoryArchiveFrame {
        sequence,
        kind,
        observed_at: time.into(),
        bytes: bytes.to_vec(),
    }
}

async fn insert_original_part(
    store: &PgControlPlaneStore,
    request: Uuid,
    part: Uuid,
    frames: &[ClientTrajectoryArchiveFrame],
    format: &str,
) {
    let mut bytes = Vec::new();
    let mut directory = Vec::new();
    for frame in frames {
        directory.push(
            json!({"sequence":frame.sequence,"kind":frame.kind,"observed_at":frame.observed_at,
            "offset":bytes.len(),"length":frame.bytes.len(),"format":format}),
        );
        bytes.extend_from_slice(&frame.bytes);
    }
    let last = frames.last().unwrap().sequence;
    let mut tx = store.pool().begin().await.unwrap();
    sqlx::query("insert into client_trajectory_archive_parts(part_id,request_id,first_sequence,last_sequence,frames,bytes) values($1,$2,$3,$4,$5,$6)")
        .bind(part).bind(request).bind(frames[0].sequence).bind(last).bind(Value::Array(directory)).bind(bytes)
        .execute(&mut *tx).await.unwrap();
    sqlx::query("update client_trajectory_archive_heads set persisted_through=greatest(persisted_through,$2) where request_id=$1")
        .bind(request).bind(last).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}

fn assert_frames(
    actual: &[ClientTrajectoryArchiveFrame],
    expected: &[ClientTrajectoryArchiveFrame],
) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.sequence, expected.sequence);
        assert_eq!(actual.kind, expected.kind);
        assert_eq!(actual.observed_at, expected.observed_at);
        assert_eq!(actual.bytes, expected.bytes);
    }
}

#[tokio::test]
async fn raw_codec_receipt_restart_late_bind_retry_scope_and_mixed_part_pages() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let request = Uuid::now_v7();
    let part = Uuid::now_v7();
    let binary = [vec![0, 255, 128], vec![b'x'; 192 * 1024]].concat();
    let first = vec![
        frame(
            0,
            ClientTrajectoryFrameKind::Request,
            " any timestamp \0 北京 \n",
            &binary,
        ),
        frame(
            0,
            ClientTrajectoryFrameKind::ResponseJson,
            "",
            b"literal\0NUL",
        ),
    ];
    let input = AppendClientTrajectoryArchiveInput {
        request_id: request,
        part_id: part,
        transport: ClientTrajectoryTransport::Websocket,
        frames: first.clone(),
    };
    let receipt = store
        .append_client_trajectory_archive(&input)
        .await
        .unwrap();
    assert_eq!(receipt.persisted_through, 2);
    // The returned receipt must already be visible through another transaction
    // owner: no ACK success may precede the durable part/head commit.
    let reopened = PgControlPlaneStore::new(pool.clone());
    let visible: (i64, i16, Value, i64) = sqlx::query_as("select h.persisted_through,p.codec_version,p.frames,p.raw_byte_length from client_trajectory_archive_heads h join client_trajectory_archive_parts p on p.request_id=h.request_id where p.part_id=$1")
        .bind(part).fetch_one(reopened.pool()).await.unwrap();
    assert_eq!(
        visible,
        (2, 1, json!([]), i64::try_from(binary.len() + 11).unwrap())
    );
    assert_eq!(
        reopened
            .append_client_trajectory_archive(&input)
            .await
            .unwrap(),
        receipt
    );
    let mut foreign = input.clone();
    foreign.request_id = Uuid::now_v7();
    assert!(format!(
        "{:#}",
        reopened
            .append_client_trajectory_archive(&foreign)
            .await
            .err()
            .unwrap()
    )
    .contains("part scope mismatch"));
    let foreign_heads: i64 = sqlx::query_scalar(
        "select count(*) from client_trajectory_archive_heads where request_id=$1",
    )
    .bind(foreign.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        foreign_heads, 0,
        "failed cross-request retry must roll back its head"
    );
    let mut wrong_transport = input.clone();
    wrong_transport.transport = ClientTrajectoryTransport::Http;
    assert!(reopened
        .append_client_trajectory_archive(&wrong_transport)
        .await
        .is_err());
    assert!(reopened
        .client_trajectory_section(flow, None, request, "raw", None, 1)
        .await
        .unwrap()
        .is_none());
    bind_request(&reopened, flow, request).await;
    let owner: Option<Uuid> = sqlx::query_scalar(
        "select flow_run_id from client_trajectory_archive_heads where request_id=$1",
    )
    .bind(request)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owner, Some(flow));
    assert!(reopened
        .client_trajectory_section(Uuid::now_v7(), None, request, "raw", None, 1)
        .await
        .unwrap()
        .is_none());
    let middle = vec![
        frame(
            3,
            ClientTrajectoryFrameKind::ResponseSse,
            " not a date ",
            b"old\0\xff",
        ),
        frame(
            4,
            ClientTrajectoryFrameKind::ResponseJson,
            "old-time",
            b"old second",
        ),
    ];
    insert_original_part(&store, request, Uuid::now_v7(), &middle, "wire").await;
    sqlx::query("update flow_runs set status='succeeded',finished_at=now() where id=$1")
        .bind(flow)
        .execute(&pool)
        .await
        .unwrap();
    let tail = frame(
        0,
        ClientTrajectoryFrameKind::ResponseSse,
        " late after EOF ",
        b"data: tail\n\n\0\xff",
    );
    assert_eq!(
        reopened
            .append_client_trajectory_archive(&AppendClientTrajectoryArchiveInput {
                request_id: request,
                part_id: Uuid::now_v7(),
                transport: ClientTrajectoryTransport::Websocket,
                frames: vec![tail.clone()],
            })
            .await
            .unwrap()
            .persisted_through,
        5
    );
    let expected = vec![
        ClientTrajectoryArchiveFrame {
            sequence: 1,
            ..first[0].clone()
        },
        ClientTrajectoryArchiveFrame {
            sequence: 2,
            ..first[1].clone()
        },
        middle[0].clone(),
        middle[1].clone(),
        ClientTrajectoryArchiveFrame {
            sequence: 5,
            ..tail
        },
    ];
    assert_frames(
        &reopened
            .read_client_trajectory_archive(request, 0, 10)
            .await
            .unwrap(),
        &expected,
    );
    assert_frames(
        &reopened
            .read_client_trajectory_archive(request, 1, 4)
            .await
            .unwrap(),
        &expected[1..],
    );
    let first_page = reopened
        .client_trajectory_section(flow, None, request, "raw", None, 2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        first_page
            .items
            .iter()
            .map(|f| f.sequence)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(first_page.items[0].value["direction"], "submitted");
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(first_page.items[0].value["body"].as_str().unwrap())
            .unwrap(),
        binary
    );
    assert_eq!(first_page.next_cursor, Some(2));
    let middle_page = reopened
        .client_trajectory_section(flow, None, request, "raw", first_page.next_cursor, 2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        middle_page
            .items
            .iter()
            .map(|f| f.sequence)
            .collect::<Vec<_>>(),
        [3, 4]
    );
    assert_eq!(middle_page.next_cursor, Some(4));
    let final_page = reopened
        .client_trajectory_section(flow, None, request, "raw", middle_page.next_cursor, 2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(final_page.items[0].sequence, 5);
    assert_eq!(final_page.items[0].value["frame_kind"], "response_sse");
    assert_eq!(final_page.next_cursor, None);

    let corrupt = Uuid::now_v7();
    reopened
        .append_client_trajectory_archive(&AppendClientTrajectoryArchiveInput {
            request_id: request,
            part_id: corrupt,
            transport: ClientTrajectoryTransport::Websocket,
            frames: vec![frame(
                0,
                ClientTrajectoryFrameKind::ResponseJson,
                "last",
                b"last",
            )],
        })
        .await
        .unwrap();
    sqlx::query("update client_trajectory_archive_parts set frame_directory=substring(frame_directory from 1 for octet_length(frame_directory)-1) where part_id=$1")
        .bind(corrupt).execute(&pool).await.unwrap();
    // Corruption in a later part cannot force a one-frame page to read the
    // entire capture; reaching that part must still reject its damaged data.
    assert_frames(
        &reopened
            .read_client_trajectory_archive(request, 0, 1)
            .await
            .unwrap(),
        &expected[..1],
    );
    assert!(reopened
        .read_client_trajectory_archive(request, 5, 1)
        .await
        .is_err());
}

#[tokio::test]
async fn raw_codec_historical_migration_reentry_exact_wire_and_legacy_sql_reference_retained() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let request = Uuid::now_v7();
    bind_request(&store, flow, request).await;
    // Capture binding does not itself manufacture an archive head.
    sqlx::query("insert into client_trajectory_archive_heads(request_id,transport,flow_run_id) values($1,'http',$2)")
        .bind(request).bind(flow).execute(&pool).await.unwrap();
    let wire = vec![
        frame(
            90,
            ClientTrajectoryFrameKind::Request,
            " unusual old timestamp ",
            b"binary\0\xff\x80",
        ),
        frame(
            91,
            ClientTrajectoryFrameKind::ResponseSse,
            AT,
            "data:  北京 🌍\n\n".as_bytes(),
        ),
    ];
    let wire_id = Uuid::now_v7();
    insert_original_part(&store, request, wire_id, &wire, "wire").await;
    let legacy_id = Uuid::now_v7();
    let legacy_value = json!({"direction":"emitted","encoding":"utf8","body":" exact legacy body \n","frame_kind":"response_json","extra":{"kept":true}});
    let original = json!({"flow_run_id":flow,"node_run_id":null,"request_id":request,"observed_at":AT,"metadata":{"exact":" kept \n"},
        "fact":{"kind":"section","step_id":request,"section":"raw","other":"fact metadata","value":legacy_value}});
    let legacy_frames = vec![frame(
        92,
        ClientTrajectoryFrameKind::ResponseJson,
        AT,
        &serde_json::to_vec(&legacy_value).unwrap(),
    )];
    insert_original_part(&store, request, legacy_id, &legacy_frames, "legacy_json").await;
    let mut stripped = original.clone();
    stripped["fact"].as_object_mut().unwrap().remove("value");
    let mut projection = stripped.clone();
    projection["_client_archive_ref"] = json!({"part_id":legacy_id,"request_id":request});
    sqlx::query("insert into runtime_events(id,flow_run_id,sequence,event_type,layer,source,trust_level,payload,raw_json_payloads,visibility,durability) values($1,$2,92,'client_protocol_trajectory','runtime_item','host','host_fact',$3,$4,'internal','durable')")
        .bind(legacy_id).bind(flow).bind(projection).bind(json!({"payload":serde_json::to_string(&stripped).unwrap()})).execute(&pool).await.unwrap();
    let before: (Value, Vec<u8>, i16) = sqlx::query_as(
        "select frames,bytes,codec_version from client_trajectory_archive_parts where part_id=$1",
    )
    .bind(legacy_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let restored: Value = sqlx::query_scalar("select runtime_event_original_payload(payload,raw_json_payloads,flow_run_id) from runtime_events where id=$1")
        .bind(legacy_id).fetch_one(&pool).await.unwrap();
    assert_eq!(restored, original);

    let nul_payload = json!({"fact":{"value":{"encoding":"utf8","body":"body\0🌍","frame_kind":"response_json"}}});
    let nul_id = Uuid::now_v7();
    insert_original_part(
        &store,
        request,
        nul_id,
        &[frame(
            93,
            ClientTrajectoryFrameKind::ResponseJson,
            AT,
            &serde_json::to_vec(&nul_payload).unwrap(),
        )],
        "legacy_payload_json",
    )
    .await;
    let second_id = Uuid::now_v7();
    let second = vec![frame(
        94,
        ClientTrajectoryFrameKind::ResponseJson,
        "late historic",
        b"exact second\0",
    )];
    insert_original_part(&store, request, second_id, &second, "wire").await;
    assert!(store
        .migrate_client_trajectory_archive_parts(Uuid::now_v7(), request)
        .await
        .is_err());
    assert!(store
        .migrate_client_trajectory_archive_parts(flow, Uuid::now_v7())
        .await
        .is_err());
    assert_eq!(
        store
            .migrate_client_trajectory_archive_parts(flow, request)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        store
            .migrate_client_trajectory_archive_parts(flow, request)
            .await
            .unwrap(),
        0
    );
    let versions: Vec<(Uuid, i16)> = sqlx::query_as("select part_id,codec_version from client_trajectory_archive_parts where request_id=$1 order by first_sequence")
        .bind(request).fetch_all(&pool).await.unwrap();
    assert_eq!(
        versions,
        [(wire_id, 1), (legacy_id, 0), (nul_id, 0), (second_id, 1)]
    );
    let after: (Value, Vec<u8>, i16) = sqlx::query_as(
        "select frames,bytes,codec_version from client_trajectory_archive_parts where part_id=$1",
    )
    .bind(legacy_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        before, after,
        "SQL legacy reference must retain the exact original layout"
    );
    let restored: Value = sqlx::query_scalar("select runtime_event_original_payload(payload,raw_json_payloads,flow_run_id) from runtime_events where id=$1")
        .bind(legacy_id).fetch_one(&pool).await.unwrap();
    assert_eq!(restored, original);
    let expected = [
        wire,
        vec![
            frame(
                92,
                ClientTrajectoryFrameKind::ResponseJson,
                AT,
                legacy_value["body"].as_str().unwrap().as_bytes(),
            ),
            frame(
                93,
                ClientTrajectoryFrameKind::ResponseJson,
                AT,
                "body\0🌍".as_bytes(),
            ),
        ],
        second,
    ]
    .concat();
    assert_frames(
        &store
            .read_client_trajectory_archive(request, 0, 128)
            .await
            .unwrap(),
        &expected,
    );
    let legacy_page = store
        .client_trajectory_section(flow, None, request, "raw", Some(91), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        legacy_page.items[0].value, legacy_value,
        "legacy section extras remain visible"
    );

    // A restart can discover a previously unconverted later part without
    // recoding earlier parts or resetting the request cursor/head.
    let later = vec![frame(
        95,
        ClientTrajectoryFrameKind::ResponseJson,
        AT,
        b"reentered\0\xff",
    )];
    insert_original_part(&store, request, Uuid::now_v7(), &later, "wire").await;
    let reopened = PgControlPlaneStore::new(pool.clone());
    assert_eq!(
        reopened
            .migrate_client_trajectory_archive_parts(flow, request)
            .await
            .unwrap(),
        1
    );
    assert_frames(
        &reopened
            .read_client_trajectory_archive(request, 94, 1)
            .await
            .unwrap(),
        &later,
    );
    let persisted: i64 = sqlx::query_scalar(
        "select persisted_through from client_trajectory_archive_heads where request_id=$1",
    )
    .bind(request)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(persisted, 95);

    let corrupt = Uuid::now_v7();
    insert_original_part(
        &store,
        request,
        corrupt,
        &[frame(
            96,
            ClientTrajectoryFrameKind::ResponseJson,
            AT,
            b"bad",
        )],
        "wire",
    )
    .await;
    sqlx::query("update client_trajectory_archive_parts set frames=jsonb_set(frames,'{0,length}','999'::jsonb) where part_id=$1")
        .bind(corrupt).execute(&pool).await.unwrap();
    assert!(reopened
        .migrate_client_trajectory_archive_parts(flow, request)
        .await
        .is_err());
    let unchanged: (i16, Vec<u8>) = sqlx::query_as(
        "select codec_version,bytes from client_trajectory_archive_parts where part_id=$1",
    )
    .bind(corrupt)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        unchanged,
        (0, b"bad".to_vec()),
        "unverifiable historical bytes may not be switched or released"
    );
}

#[tokio::test]
async fn raw_codec_database_corruption_unknown_version_raw_length_checksum_and_truncation_rejected()
{
    let (pool, _) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let request = Uuid::now_v7();
    let part = Uuid::now_v7();
    store
        .append_client_trajectory_archive(&AppendClientTrajectoryArchiveInput {
            request_id: request,
            part_id: part,
            transport: ClientTrajectoryTransport::Http,
            frames: vec![frame(
                0,
                ClientTrajectoryFrameKind::Request,
                "raw time",
                b"exact\0\xff",
            )],
        })
        .await
        .unwrap();
    let original: (Vec<u8>, Vec<u8>, i64, Vec<u8>) = sqlx::query_as("select bytes,frame_directory,raw_byte_length,raw_checksum from client_trajectory_archive_parts where part_id=$1")
        .bind(part).fetch_one(&pool).await.unwrap();
    for (sql, expected) in [
        ("update client_trajectory_archive_parts set codec_version=99 where part_id=$1", "unknown client archive codec version"),
        ("update client_trajectory_archive_parts set raw_byte_length=raw_byte_length+1 where part_id=$1", "raw length mismatch"),
        ("update client_trajectory_archive_parts set raw_checksum=decode(repeat('00',32),'hex') where part_id=$1", "checksum mismatch"),
        ("update client_trajectory_archive_parts set bytes=substring(bytes from 1 for octet_length(bytes)-1) where part_id=$1", "truncated"),
        ("update client_trajectory_archive_parts set frame_directory=substring(frame_directory from 1 for octet_length(frame_directory)-1) where part_id=$1", "checksum mismatch"),
    ] {
        sqlx::query(sql).bind(part).execute(&pool).await.unwrap();
        let error = store.read_client_trajectory_archive(request, 0, 1).await.err().expect("damaged part was accepted");
        assert!(format!("{error:#}").contains(expected), "{error:#}");
        sqlx::query("update client_trajectory_archive_parts set codec_version=1,bytes=$2,frame_directory=$3,raw_byte_length=$4,raw_checksum=$5 where part_id=$1")
            .bind(part).bind(&original.0).bind(&original.1).bind(original.2).bind(&original.3).execute(&pool).await.unwrap();
    }
    let restored = store
        .read_client_trajectory_archive(request, 0, 1)
        .await
        .unwrap();
    assert_eq!(restored[0].bytes, b"exact\0\xff");
}

#[tokio::test]
async fn raw_history_prefilter_retains_unreferenced_legacy_and_rejects_unknown_format() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let request = Uuid::now_v7();
    bind_request(&store, flow, request).await;
    sqlx::query("insert into client_trajectory_archive_heads(request_id,transport,flow_run_id) values($1,'http',$2)")
        .bind(request).bind(flow).execute(&pool).await.unwrap();
    for (sequence, format) in [(10, "legacy_json"), (11, "legacy_payload_json")] {
        let body =
            json!({"fact":{"value":{"body":"retained original"}},"body":"retained original"});
        insert_original_part(
            &store,
            request,
            Uuid::now_v7(),
            &[frame(
                sequence,
                ClientTrajectoryFrameKind::ResponseJson,
                AT,
                &serde_json::to_vec(&body).unwrap(),
            )],
            format,
        )
        .await;
    }
    let before: Vec<(Uuid, Value, Vec<u8>, i16)> = sqlx::query_as("select part_id,frames,bytes,codec_version from client_trajectory_archive_parts where request_id=$1 order by first_sequence")
        .bind(request).fetch_all(&pool).await.unwrap();
    assert_eq!(
        store
            .migrate_client_trajectory_archive_parts(flow, request)
            .await
            .unwrap(),
        0
    );
    let after: Vec<(Uuid, Value, Vec<u8>, i16)> = sqlx::query_as("select part_id,frames,bytes,codec_version from client_trajectory_archive_parts where request_id=$1 order by first_sequence")
        .bind(request).fetch_all(&pool).await.unwrap();
    assert_eq!(
        before, after,
        "unreferenced known legacy layouts must remain byte-for-byte intact"
    );
    let unknown = Uuid::now_v7();
    insert_original_part(
        &store,
        request,
        unknown,
        &[frame(
            12,
            ClientTrajectoryFrameKind::ResponseJson,
            AT,
            b"unknown original",
        )],
        "future_format",
    )
    .await;
    let error = store
        .migrate_client_trajectory_archive_parts(flow, request)
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("archive original frame format unknown"));
    let version: i16 = sqlx::query_scalar(
        "select codec_version from client_trajectory_archive_parts where part_id=$1",
    )
    .bind(unknown)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(version, 0);
    sqlx::query("delete from client_trajectory_archive_parts where part_id=$1")
        .bind(unknown)
        .execute(&pool)
        .await
        .unwrap();
    for known in ["legacy_json", "legacy_payload_json"] {
        let mixed = Uuid::now_v7();
        let body =
            serde_json::to_vec(&json!({"fact":{"value":{"body":"kept"}},"body":"kept"})).unwrap();
        insert_original_part(
            &store,
            request,
            mixed,
            &[
                frame(12, ClientTrajectoryFrameKind::ResponseJson, AT, &body),
                frame(
                    13,
                    ClientTrajectoryFrameKind::ResponseJson,
                    AT,
                    b"future bytes",
                ),
            ],
            known,
        )
        .await;
        sqlx::query("update client_trajectory_archive_parts set frames=jsonb_set(frames,'{1,format}','\"future_format\"'::jsonb) where part_id=$1")
            .bind(mixed).execute(&pool).await.unwrap();
        let before: (Value, Vec<u8>, i16) = sqlx::query_as("select frames,bytes,codec_version from client_trajectory_archive_parts where part_id=$1")
            .bind(mixed).fetch_one(&pool).await.unwrap();
        let error = store
            .migrate_client_trajectory_archive_parts(flow, request)
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("archive original frame format unknown"),
            "mixed {known} bypassed strict decoding"
        );
        let after: (Value, Vec<u8>, i16) = sqlx::query_as("select frames,bytes,codec_version from client_trajectory_archive_parts where part_id=$1")
            .bind(mixed).fetch_one(&pool).await.unwrap();
        assert_eq!(before, after);
        sqlx::query("delete from client_trajectory_archive_parts where part_id=$1")
            .bind(mixed)
            .execute(&pool)
            .await
            .unwrap();
    }
}
