use super::*;

async fn seed() -> (PgControlPlaneStore, Uuid) {
    let (pool, run) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    (PgControlPlaneStore::new(pool), run)
}

fn defaults(run: Uuid, request: Uuid, id: Uuid) -> ClientTrajectoryStep {
    let mut item = step(run, None, request, id, "submitted", "request");
    item.name = item.category.clone();
    item.preview = item.name.clone();
    item.result_preview = Some(item.preview.clone());
    item.parameters_preview = None;
    item.call_id = None;
    item.parent_id = Some(request);
    item
}

async fn add(store: &PgControlPlaneStore, run: Uuid, request: Uuid, item: ClientTrajectoryStep) {
    append(
        store,
        run,
        None,
        request,
        ClientTrajectoryFact::Step {
            step: Box::new(item),
        },
    )
    .await;
}

async fn restored(store: &PgControlPlaneStore, id: Uuid) -> Value {
    sqlx::query_scalar("select client_trajectory_step_storage_body(id,flow_run_id) from client_trajectory_steps where id=$1")
        .bind(id).fetch_one(store.pool()).await.unwrap()
}

async fn physical(store: &PgControlPlaneStore, id: Uuid) -> (Value, i16) {
    sqlx::query_as("select metadata,metadata_layout from client_trajectory_steps where id=$1")
        .bind(id)
        .fetch_one(store.pool())
        .await
        .unwrap()
}

async fn install_layout_zero(
    store: &PgControlPlaneStore,
    id: Uuid,
    original: &Value,
    compact: bool,
) {
    let mut body = original.clone();
    if compact {
        for key in ["id", "request_id", "flow_run_id", "node_run_id"] {
            body.as_object_mut().unwrap().remove(key);
        }
    }
    // Use the original sidecar even when NUL or arbitrary numeric tokens cannot
    // survive a PostgreSQL JSONB projection. Preserve an unrelated sidecar too.
    sqlx::query("update client_trajectory_steps set metadata=$2,raw_json_payloads=$3,metadata_compact=$4,metadata_layout=0 where id=$1")
        .bind(id).bind(json!({"category":"request","origin":"submitted","available_sections":[]}))
        .bind(json!({"metadata":serde_json::to_string(&body).unwrap(),"fixture":"untouched"}))
        .bind(compact).execute(store.pool()).await.unwrap();
}

#[tokio::test]
async fn client_dense_directory_writer_defaults_and_original_equality_chains_are_lossless() {
    let (store, run) = seed().await;
    let request = Uuid::now_v7();
    begin(&store, run, None, request).await;
    let item = defaults(run, request, request);
    let expected = serde_json::to_value(&item).unwrap();
    add(&store, run, request, item).await;
    let (body, layout) = physical(&store, request).await;
    assert_eq!(layout, 1);
    assert_eq!(
        body,
        json!({"category":"request","origin":"submitted","available_sections":expected["available_sections"]})
    );
    assert_eq!(restored(&store, request).await, expected);

    // Each equality is independent of earlier omitted defaults, and NUL in a
    // surviving source must be restored before rebuilding its equal children.
    let id = Uuid::now_v7();
    let mut item = defaults(run, request, id);
    item.name = "original\0name".into();
    item.preview = item.name.clone();
    item.result_preview = Some(item.preview.clone());
    let expected = serde_json::to_value(&item).unwrap();
    add(&store, run, request, item.clone()).await;
    let (body, layout) = physical(&store, id).await;
    assert_eq!(layout, 1);
    assert!(body.get("name").is_some());
    assert!(body.get("preview").is_none());
    assert!(body.get("result_preview").is_none());
    assert_eq!(restored(&store, id).await, expected);
    item.preview = "different\0preview".into();
    item.result_preview = Some(item.preview.clone());
    let revised = serde_json::to_value(&item).unwrap();
    add(&store, run, request, item).await;
    assert_eq!(restored(&store, id).await, revised);
    let page = store
        .client_trajectory_page(run, None, None, 20)
        .await
        .unwrap();
    let item = page.items.iter().find(|item| item.id == id).unwrap();
    assert_eq!(item.name, "original\0name");
    assert_eq!(item.preview, "different\0preview");
    assert_eq!(item.result_preview.as_deref(), Some("different\0preview"));
}

#[tokio::test]
async fn client_dense_directory_nondefaults_and_explicit_nulls_remain_original() {
    let (store, run) = seed().await;
    let request = Uuid::now_v7();
    begin(&store, run, None, request).await;
    add(&store, run, request, defaults(run, request, request)).await;
    let mut original = restored(&store, request).await;
    for (key, value) in [
        ("namespace", json!("ns\0")),
        ("parameters_preview", json!("params")),
        ("call_id", json!("call")),
        ("item_id", json!("item")),
        ("response_id", json!("response")),
        ("turn_id", json!("turn")),
        ("related_step_id", json!(Uuid::now_v7())),
        ("sequence", json!(991)),
        ("protocol", json!("other")),
        ("transport", json!("websocket")),
        ("status", json!("recorded")),
        ("created_at", json!("different\0time")),
        ("parent_id", Value::Null),
        ("name", json!("different name")),
        ("preview", json!("different preview")),
        ("result_preview", Value::Null),
    ] {
        original[key] = value;
    }
    install_layout_zero(&store, request, &original, false).await;
    assert_eq!(
        store
            .compact_client_trajectory_directories(&[run], 20)
            .await
            .unwrap(),
        1
    );
    assert_eq!(restored(&store, request).await, original);
    let (body, layout) = physical(&store, request).await;
    assert_eq!(layout, 1);
    for key in [
        "namespace",
        "parameters_preview",
        "call_id",
        "item_id",
        "response_id",
        "turn_id",
        "related_step_id",
        "sequence",
        "protocol",
        "transport",
        "status",
        "created_at",
        "parent_id",
        "name",
        "preview",
        "result_preview",
    ] {
        assert!(body.get(key).is_some(), "nondefault {key} retained");
    }
    for key in [
        "sequence",
        "protocol",
        "transport",
        "status",
        "created_at",
        "name",
    ] {
        original[key] = Value::Null;
    }
    install_layout_zero(&store, request, &original, true).await;
    assert_eq!(
        store
            .compact_client_trajectory_directories(&[run], 20)
            .await
            .unwrap(),
        1
    );
    assert_eq!(restored(&store, request).await, original);
    let (body, _) = physical(&store, request).await;
    for key in [
        "sequence",
        "protocol",
        "transport",
        "status",
        "created_at",
        "parent_id",
        "name",
        "result_preview",
    ] {
        assert_eq!(
            body.get(key),
            Some(&Value::Null),
            "explicit null {key} retained"
        );
    }
}

#[tokio::test]
async fn client_dense_directory_history_upgrades_old_compact_preserves_unknown_tokens_and_skips_partial(
) {
    let (store, run) = seed().await;
    let request = Uuid::now_v7();
    begin(&store, run, None, request).await;
    add(&store, run, request, defaults(run, request, request)).await;
    let mut original = restored(&store, request).await;
    original["unknown\0key"] = json!({"nul":"literal\0","escape":"literal\\u0000"});
    let number_token = "9007199254740993";
    original["unknown_number"] = serde_json::from_str(number_token).unwrap();
    install_layout_zero(&store, request, &original, true).await;
    let partial_id = Uuid::now_v7();
    add(&store, run, request, defaults(run, request, partial_id)).await;
    let mut partial = restored(&store, partial_id).await;
    partial.as_object_mut().unwrap().remove("namespace");
    install_layout_zero(&store, partial_id, &partial, false).await;
    assert_eq!(
        store
            .compact_client_trajectory_directories(&[run, run], 1)
            .await
            .unwrap(),
        1
    );
    assert_eq!(restored(&store, request).await, original);
    let text: String = sqlx::query_scalar("select client_trajectory_step_storage_body(id,flow_run_id)::text from client_trajectory_steps where id=$1")
        .bind(request).fetch_one(store.pool()).await.unwrap();
    assert!(
        text.contains(number_token),
        "original numeric token retained"
    );
    let sidecar: Value =
        sqlx::query_scalar("select raw_json_payloads from client_trajectory_steps where id=$1")
            .bind(request)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(sidecar["fixture"], "untouched");
    assert_eq!(
        store
            .compact_client_trajectory_directories(&[run], 20)
            .await
            .unwrap(),
        1,
        "partial legacy shape still receives the existing four-identity compaction once"
    );
    assert_eq!(physical(&store, partial_id).await.1, 0);
    assert_eq!(restored(&store, partial_id).await, partial);
    assert_eq!(
        store
            .compact_client_trajectory_directories(&[run], 20)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .compact_client_trajectory_directories(&[], 20)
            .await
            .unwrap(),
        0
    );

    // A skipped partial original must not hide a later eligible row when the
    // scan page is only one row wide.
    let later_id = Uuid::now_v7();
    add(&store, run, request, defaults(run, request, later_id)).await;
    let later = restored(&store, later_id).await;
    install_layout_zero(&store, later_id, &later, true).await;
    assert_eq!(
        store
            .compact_client_trajectory_directories(&[run], 1)
            .await
            .unwrap(),
        1
    );
    assert_eq!(physical(&store, later_id).await.1, 1);
    assert_eq!(restored(&store, later_id).await, later);

    let other_run = Uuid::now_v7();
    sqlx::query("insert into flow_runs(id,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by) select $1,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by from flow_runs where id=$2")
        .bind(other_run).bind(run).execute(store.pool()).await.unwrap();
    let other_request = Uuid::now_v7();
    begin(&store, other_run, None, other_request).await;
    add(
        &store,
        other_run,
        other_request,
        defaults(other_run, other_request, other_request),
    )
    .await;
    let other = restored(&store, other_request).await;
    install_layout_zero(&store, other_request, &other, true).await;
    assert_eq!(
        store
            .compact_client_trajectory_directories(&[run], 20)
            .await
            .unwrap(),
        0
    );
    assert_eq!(physical(&store, other_request).await.1, 0);
    assert_eq!(restored(&store, other_request).await, other);
}

#[tokio::test]
async fn client_dense_directory_history_identity_mismatch_rolls_back_previous_upgrade() {
    let (store, run) = seed().await;
    let request = Uuid::now_v7();
    begin(&store, run, None, request).await;
    add(&store, run, request, defaults(run, request, request)).await;
    let original = restored(&store, request).await;
    install_layout_zero(&store, request, &original, true).await;
    let id = Uuid::now_v7();
    add(&store, run, request, defaults(run, request, id)).await;
    let mut wrong = restored(&store, id).await;
    wrong["request_id"] = json!(Uuid::now_v7());
    install_layout_zero(&store, id, &wrong, false).await;
    assert!(store
        .compact_client_trajectory_directories(&[run], 20)
        .await
        .is_err());
    assert_eq!(physical(&store, request).await.1, 0);
    assert_eq!(restored(&store, request).await, original);
    assert_eq!(physical(&store, id).await.1, 0);
}

#[tokio::test]
async fn client_dense_directory_legacy_semantic_mover_uses_full_original_and_resets_layout() {
    let (store, run) = seed().await;
    let request = Uuid::now_v7();
    begin(&store, run, None, request).await;
    add(&store, run, request, defaults(run, request, request)).await;
    assert_eq!(physical(&store, request).await.1, 1);
    let item = defaults(run, request, request);
    let input = AppendClientTrajectoryInput {
        flow_run_id: run,
        node_run_id: None,
        request_id: request,
        observed_at: AT.into(),
        fact: ClientTrajectoryFact::Step {
            step: Box::new(item),
        },
    };
    let mut original = serde_json::to_value(&input).unwrap();
    original["fact"]["step"]["unknown\0key"] = json!(["keep\0", 9007199254740993_u64]);
    let projection = serde_json::to_value(&input).unwrap();
    let anchor = Uuid::now_v7();
    sqlx::query("insert into runtime_events(id,flow_run_id,node_run_id,sequence,event_type,layer,source,trust_level,payload,raw_json_payloads,visibility,durability) select $1,id,NULL,runtime_event_sequence_high_water+1,'client_protocol_trajectory','runtime_item','host','host_fact',$2,$3,'internal','durable' from flow_runs where id=$4")
        .bind(anchor).bind(projection)
        .bind(json!({"payload":serde_json::to_string(&original).unwrap()})).bind(run)
        .execute(store.pool()).await.unwrap();
    assert_eq!(
        physical(&store, request).await.1,
        0,
        "legacy trigger resets dense marker"
    );
    assert_eq!(
        store
            .migrate_client_trajectory_semantic_history(&[run], 20)
            .await
            .unwrap(),
        1
    );
    assert_eq!(physical(&store, request).await.1, 1);
    assert_eq!(restored(&store, request).await, original["fact"]["step"]);
    let retained: Option<Uuid> =
        sqlx::query_scalar("select event_id from client_trajectory_steps where id=$1")
            .bind(request)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(retained, Some(anchor));
    assert_eq!(
        store
            .migrate_client_trajectory_semantic_history(&[run], 20)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn client_dense_directory_sql_restoration_preserves_unparsed_numeric_and_nul_tokens() {
    let (store, run) = seed().await;
    let request = Uuid::now_v7();
    begin(&store, run, None, request).await;
    add(&store, run, request, defaults(run, request, request)).await;
    // Install already-dense original JSON text directly. No Rust Value parse or
    // serialization touches these numeric tokens before the SQL restoration.
    let token = "1234567890123456789012345678901234567890.12345000000000000000001";
    let body = format!(
        r#"{{"category":"request","origin":"submitted","available_sections":[],"unknown\u0000key":"keep\u0000","unknown_number":{token},"unknown_exponent":1.234500e+100}}"#
    );
    let observed = r#""clock\u0000""#;
    sqlx::query("update client_trajectory_steps set metadata=$2,raw_json_payloads=$3 where id=$1")
        .bind(request)
        .bind(json!({"category":"request","origin":"submitted","available_sections":[]}))
        .bind(json!({"metadata":body,"observed_at":observed}))
        .execute(store.pool())
        .await
        .unwrap();
    let text: String = sqlx::query_scalar("select client_trajectory_step_storage_body(id,flow_run_id)::text from client_trajectory_steps where id=$1")
        .bind(request).fetch_one(store.pool()).await.unwrap();
    assert!(text.contains(token));
    assert!(text.contains("1.234500e+100"));
    let restored: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(restored["unknown\0key"], "keep\0");
    assert_eq!(restored["created_at"], "clock\0");
    assert_eq!(restored["name"], "request");
    assert_eq!(restored["preview"], "request");
    assert_eq!(restored["result_preview"], "request");
    assert_eq!(restored["parent_id"], json!(request));
}
