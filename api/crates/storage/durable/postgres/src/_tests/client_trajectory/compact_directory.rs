use super::*;

async fn fixture() -> (PgControlPlaneStore, Uuid, Uuid) {
    let (pool, run) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let request = Uuid::now_v7();
    begin(&store, run, None, request).await;
    let mut call = step(run, None, request, request, "emitted", "tool_call");
    call.name = "original\0name".into();
    call.namespace = Some("namespace\0".into());
    call.call_id = Some("call\0id".into());
    call.sequence = 777;
    append(
        &store,
        run,
        None,
        request,
        ClientTrajectoryFact::Step {
            step: Box::new(call),
        },
    )
    .await;
    section(
        &store,
        run,
        None,
        request,
        request,
        "overview",
        json!({"arguments":"actual\0", "content":[1.25, 9007199254740993_u64], "key\0":"original"}),
    )
    .await;
    section(
        &store,
        run,
        None,
        request,
        request,
        "parameters",
        json!("actual\0"),
    )
    .await;
    (store, run, request)
}

async fn legacy_directories(store: &PgControlPlaneStore, request: Uuid) {
    // Restore old full metadata and text hash without changing logical values.
    let original: Value = sqlx::query_scalar("select client_trajectory_step_original_metadata(metadata,raw_json_payloads,id,request_id,flow_run_id,node_run_id,metadata_compact) from client_trajectory_steps where id=$1")
        .bind(request).fetch_one(store.pool()).await.unwrap();
    let originals = json!({"metadata":serde_json::to_string(&original).unwrap()});
    // A safe JSONB projection is intentionally different from the original.
    sqlx::query("update client_trajectory_steps set metadata=$2,raw_json_payloads=$3,metadata_compact=false where id=$1")
        .bind(request).bind(json!({"name":"safe projection"})).bind(originals).execute(store.pool()).await.unwrap();
    sqlx::query("update client_trajectory_sections set value_hash='sha256:'||encode(value_digest,'hex'),value_digest=NULL where step_id=$1")
        .bind(request).execute(store.pool()).await.unwrap();
}

#[tokio::test]
async fn client_directory_compaction_mixed_layout_preserves_originals_links_and_reentry() {
    let (store, run, request) = fixture().await;
    let physical: (Value, bool) =
        sqlx::query_as("select metadata,metadata_compact from client_trajectory_steps where id=$1")
            .bind(request)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(physical.1);
    for key in ["id", "request_id", "flow_run_id", "node_run_id"] {
        assert!(physical.0.get(key).is_none());
    }
    assert_eq!(physical.0["sequence"], json!(777));
    let digest_lengths: Vec<(Option<String>, i32)> = sqlx::query_as("select value_hash,octet_length(value_digest) from client_trajectory_sections where step_id=$1")
        .bind(request).fetch_all(store.pool()).await.unwrap();
    assert!(digest_lengths
        .iter()
        .all(|(hash, len)| hash.is_none() && *len == 32));
    let before = store
        .client_trajectory_page(run, None, None, 20)
        .await
        .unwrap();
    let overview = store
        .client_trajectory_section(run, None, request, "overview", None, 20)
        .await
        .unwrap()
        .unwrap();
    let parameters = store
        .client_trajectory_section(run, None, request, "parameters", None, 20)
        .await
        .unwrap()
        .unwrap();
    let candidates: Vec<(Uuid, Vec<String>)> = sqlx::query_as("select content_id,content_path from client_trajectory_sections where step_id=$1 order by event_sequence")
        .bind(request).fetch_all(store.pool()).await.unwrap();
    assert_eq!(candidates[0].0, candidates[1].0);
    assert_eq!(candidates[1].1, vec!["arguments"]);
    legacy_directories(&store, request).await;
    let excluded_run = Uuid::now_v7();
    sqlx::query("insert into flow_runs(id,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by) select $1,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by from flow_runs where id=$2")
        .bind(excluded_run).bind(run).execute(store.pool()).await.unwrap();
    let excluded_request = Uuid::now_v7();
    begin(&store, excluded_run, None, excluded_request).await;
    append(
        &store,
        excluded_run,
        None,
        excluded_request,
        ClientTrajectoryFact::Step {
            step: Box::new(step(
                excluded_run,
                None,
                excluded_request,
                excluded_request,
                "emitted",
                "tool_call",
            )),
        },
    )
    .await;
    legacy_directories(&store, excluded_request).await;
    let other_request = Uuid::now_v7();
    begin(&store, run, None, other_request).await;
    let mut result = step(
        run,
        None,
        other_request,
        other_request,
        "submitted",
        "tool_result",
    );
    result.call_id = Some("call\0id".into());
    append(
        &store,
        run,
        None,
        other_request,
        ClientTrajectoryFact::Step {
            step: Box::new(result),
        },
    )
    .await;
    let mut touched = 0;
    loop {
        let count = store
            .compact_client_trajectory_directories(&[run, run], 1)
            .await
            .unwrap();
        if count == 0 {
            break;
        }
        touched += count;
    }
    assert_eq!(touched, 3);
    let excluded_compact: bool =
        sqlx::query_scalar("select metadata_compact from client_trajectory_steps where id=$1")
            .bind(excluded_request)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(
        !excluded_compact,
        "a valid run outside the allowlist stays untouched"
    );
    assert_eq!(
        store
            .compact_client_trajectory_directories(&[run], 10)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .compact_client_trajectory_directories(&[], 10)
            .await
            .unwrap(),
        0
    );
    let after = store
        .client_trajectory_page(run, None, None, 20)
        .await
        .unwrap();
    let call = after.items.iter().find(|item| item.id == request).unwrap();
    assert_eq!(
        serde_json::to_value(call).unwrap(),
        serde_json::to_value(&before.items[0]).unwrap()
    );
    let result = after
        .items
        .iter()
        .find(|item| item.id == other_request)
        .unwrap();
    assert_eq!(result.related_step_id, Some(request));
    assert_eq!(result.namespace.as_deref(), Some("namespace\0"));
    for (name, prior) in [("overview", overview), ("parameters", parameters)] {
        let after = store
            .client_trajectory_section(run, None, request, name, None, 20)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::to_value(after).unwrap(),
            serde_json::to_value(prior).unwrap()
        );
    }
    assert!(store
        .client_trajectory_section(Uuid::now_v7(), None, request, "parameters", None, 20)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn client_directory_compaction_rejects_hash_corruption_and_rolls_back_step() {
    let (store, run, request) = fixture().await;
    legacy_directories(&store, request).await;
    sqlx::query("update client_trajectory_sections set value_hash=$2 where step_id=$1 and section='parameters'")
        .bind(request).bind(format!("sha256:{}", "0".repeat(64))).execute(store.pool()).await.unwrap();
    assert!(store
        .compact_client_trajectory_directories(&[run], 10)
        .await
        .is_err());
    let compact: bool =
        sqlx::query_scalar("select metadata_compact from client_trajectory_steps where id=$1")
            .bind(request)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(
        !compact,
        "all earlier updates roll back with the corrupt section"
    );
    let digest_count: i64 = sqlx::query_scalar("select count(*) from client_trajectory_sections where step_id=$1 and value_digest is not null")
        .bind(request).fetch_one(store.pool()).await.unwrap();
    assert_eq!(digest_count, 0);
}

#[tokio::test]
async fn client_directory_compaction_rejects_original_identity_and_scoped_section_mismatch() {
    let (store, run, request) = fixture().await;
    legacy_directories(&store, request).await;
    let original: Value =
        sqlx::query_scalar("select raw_json_payloads from client_trajectory_steps where id=$1")
            .bind(request)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let mut wrong: Value = serde_json::from_str(original["metadata"].as_str().unwrap()).unwrap();
    wrong["request_id"] = json!(Uuid::now_v7());
    sqlx::query("update client_trajectory_steps set raw_json_payloads=$2 where id=$1")
        .bind(request)
        .bind(json!({"metadata":serde_json::to_string(&wrong).unwrap()}))
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .compact_client_trajectory_directories(&[run], 10)
        .await
        .is_err());
    sqlx::query("update client_trajectory_steps set raw_json_payloads=$2 where id=$1")
        .bind(request)
        .bind(original)
        .execute(store.pool())
        .await
        .unwrap();
    let other = Uuid::now_v7();
    begin(&store, run, None, other).await;
    sqlx::query("update client_trajectory_sections set request_id=$2 where step_id=$1 and section='parameters'")
        .bind(request).bind(other).execute(store.pool()).await.unwrap();
    assert!(store
        .compact_client_trajectory_directories(&[run], 10)
        .await
        .is_err());
    assert!(store
        .compact_client_trajectory_directories(&[Uuid::now_v7()], 10)
        .await
        .is_err());
    assert!(store
        .compact_client_trajectory_directories(&[run], 0)
        .await
        .is_err());
}
