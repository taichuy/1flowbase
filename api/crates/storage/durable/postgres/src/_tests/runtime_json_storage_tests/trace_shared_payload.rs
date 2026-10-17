use super::*;
use sqlx::Row;

fn trace_node(trace: Uuid) -> ApplicationRunTraceNodeProjectionInput {
    ApplicationRunTraceNodeProjectionInput {
        trace_node_id: trace,
        parent_trace_node_id: None,
        stable_locator: "run:shared-write-fixture".into(),
        node_kind: "tool_callback".into(),
        owner_kind: Some("tool_call".into()),
        owner_id: Some("call-shared".into()),
        order_key: "000001".into(),
        node_id: None,
        node_type: Some("tool".into()),
        node_mode: None,
        node_alias: "Shared write fixture".into(),
        status: "returned".into(),
        started_at: OffsetDateTime::UNIX_EPOCH,
        finished_at: None,
        duration_ms: None,
        metrics_payload: json!({}),
        has_children: false,
        child_count: 0,
        has_content: true,
        content_ref: None,
        source_flow_run_id: None,
        source_trace_node_id: None,
        parent_callback_task_id: None,
        parent_tool_call_id: None,
        trace_relation_kind: None,
    }
}

#[tokio::test]
async fn shared_trace_write_payload_uses_the_existing_lossless_projection_and_raw_sidecar() {
    let (store, run, _, _) = seed().await;
    let trace = Uuid::now_v7();
    let mut shared =
        TraceProjectionWritePayload::from(json!({"id":"call-shared","callback_task_id":null}));
    shared.set_shared_fields(
        &["request_payload", "tool_call"],
        json!({
            "nul\\u0000key":"literal", "nul\0key":"before\0after",
            "arguments":[null, true, 1.25, i64::MIN, u64::MAX, "\\", "\\u0000"],
        }),
    );
    shared.set_shared_fields(
        &["callback_payload", "parsed_result", "tool_result"],
        json!([
            {"a\0b":"result\0text", "a\\u0000b":"backslash collision"}, null, 1e20,
        ]),
    );
    let cases = vec![
        (shared, true),
        (TraceProjectionWritePayload::from(Value::Null), false),
        (
            json!([null, false, i64::MIN, u64::MAX, 1.25, 1e20]).into(),
            false,
        ),
        (
            json!({"ordinary":"\\u0000","nul\\u0000key":null}).into(),
            false,
        ),
        (
            json!({"nul\0key":"nul\0value","nul\\u0000key":"distinct"}).into(),
            true,
        ),
        (json!("scalar\0string").into(), true),
    ];
    let source_refs =
        json!([{"kind":"fixture\0kind","locator":"literal\\u0000","key\0nul":"value"}]);
    for (index, (payload, needs_original)) in cases.into_iter().enumerate() {
        let expected = serde_json::to_value(&payload).unwrap();
        store
            .replace_application_run_trace_projection(&ReplaceApplicationRunTraceProjectionInput {
                flow_run_id: run,
                projection_version: 15,
                source_watermark: format!("shared-write-{index}"),
                nodes: vec![trace_node(trace)],
                contents: vec![ApplicationRunTraceNodeContentProjectionInput {
                    trace_node_id: trace,
                    content_kind: "tool_callback".into(),
                    payload,
                    source_refs: source_refs.clone(),
                }],
            })
            .await
            .unwrap();
        let content = store
            .get_application_run_trace_node_content(run, trace)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(content.payload, expected);
        assert_eq!(content.source_refs, source_refs);
        let status = store
            .get_application_run_trace_projection_status(run, 15)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status.projection_version, 15);
        assert_eq!(status.source_watermark, format!("shared-write-{index}"));
        let row = sqlx::query("select payload,raw_json_payloads from application_run_trace_node_contents where flow_run_id=$1 and trace_node_id=$2")
            .bind(run).bind(trace).fetch_one(store.pool()).await.unwrap();
        let raw: Value = row.get("raw_json_payloads");
        if needs_original {
            let decoded: Value = serde_json::from_str(raw["payload"].as_str().unwrap()).unwrap();
            assert_eq!(decoded, expected);
        } else {
            assert!(
                raw.get("payload").is_none(),
                "ordinary replacement removes the payload sidecar"
            );
            assert_eq!(row.get::<Value, _>("payload"), expected);
        }
        let raw_refs: Value = serde_json::from_str(raw["source_refs"].as_str().unwrap()).unwrap();
        assert_eq!(raw_refs, source_refs);
    }
}
