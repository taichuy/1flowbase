use super::*;

#[test]
fn root_1569_ac_006_inline_budget_counts_unicode_and_valid_json_without_truncation() {
    use crate::routes::mcp_protocol::result_delivery::{
        exceeds_inline_limit, inline_limit, DEFAULT_INLINE_CHARS, MAX_INLINE_CHARS,
    };

    let exact = json!("界".repeat(DEFAULT_INLINE_CHARS - 2));
    let over = json!("界".repeat(DEFAULT_INLINE_CHARS - 1));
    assert!(!exceeds_inline_limit(&exact, DEFAULT_INLINE_CHARS));
    assert!(exceeds_inline_limit(&over, DEFAULT_INLINE_CHARS));
    assert!(!exceeds_inline_limit(
        &json!({"nested": [{"value": "界"}, [1, 2, 3]]}),
        DEFAULT_INLINE_CHARS
    ));
    assert_eq!(inline_limit(&json!({})), Ok(DEFAULT_INLINE_CHARS));
    assert_eq!(
        inline_limit(&json!({"max_inline_chars": MAX_INLINE_CHARS})),
        Ok(MAX_INLINE_CHARS)
    );
    assert_eq!(
        inline_limit(&json!({"max_inline_chars": 80_000})),
        Ok(80_000)
    );
    assert!(inline_limit(&json!({"max_inline_chars": 0})).is_err());
}

#[tokio::test]
async fn root_1569_ac_006_ac_008_oversized_read_uses_read_only_paged_continuation() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    let token = create_api_key(&app, &cookie, &csrf).await;
    create_interface_tool_and_binding(
        &app,
        &cookie,
        &csrf,
        "catalog_continuation",
        "get_mcp_catalog",
        json!({ "mappings": [] }),
    )
    .await;

    let oversized = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":30,
            "method":"tools/call",
            "params":{
                "name":"mcp_call",
                "arguments":{
                    "tool_id":"catalog_continuation",
                    "arguments":{},
                    "max_inline_chars":1
                }
            }
        }),
    )
    .await;
    let compact = &oversized["result"]["structuredContent"];
    assert_eq!(compact["outcome"], json!("succeeded"));
    assert_eq!(compact["operation_id"], json!("get_mcp_catalog"));
    assert_eq!(compact["detail"]["status"], json!("continuation_available"));
    assert_eq!(compact["retry_original"], json!(false));
    assert!(compact.get("receipt_id").is_none());
    let result_ref = compact["detail"]["result_ref"].as_str().unwrap();
    let initial_cursor = compact["detail"]["next_cursor"]
        .as_str()
        .expect("continuation receipt should expose the first opaque cursor");

    let missing_cursor = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":301,
            "method":"tools/call",
            "params":{
                "name":"mcp_result",
                "arguments":{"result_ref":result_ref}
            }
        }),
    )
    .await;
    assert_eq!(
        missing_cursor["result"]["structuredContent"]["detail_status"],
        json!("available")
    );

    let first_page = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":31,
            "method":"tools/call",
            "params":{
                "name":"mcp_result",
                "arguments":{
                    "result_ref":result_ref,
                    "cursor":initial_cursor,
                    "max_inline_chars":1000
                }
            }
        }),
    )
    .await;
    let page = &first_page["result"]["structuredContent"];
    assert_eq!(page["detail_status"], json!("available"));
    assert_eq!(page["retry_original"], json!(false));
    assert!(page["entries"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));
    assert!(page["entries"]
        .as_array()
        .unwrap()
        .iter()
        .all(|entry| entry["path"].is_string() && entry.get("value").is_some()));
    let next_cursor = page["next_cursor"]
        .as_str()
        .expect("the finite fixture should require more than one continuation page");
    let second_page = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":311,
            "method":"tools/call",
            "params":{
                "name":"mcp_result",
                "arguments":{
                    "result_ref":result_ref,
                    "cursor":next_cursor,
                    "max_inline_chars":1000
                }
            }
        }),
    )
    .await;
    assert_eq!(
        second_page["result"]["structuredContent"]["detail_status"],
        json!("available")
    );

    let cache_key = format!("mcp-result:{}:{}", state.bootstrap_workspace_id, result_ref);
    state
        .infrastructure
        .cache_store()
        .delete(&cache_key)
        .await
        .unwrap();
    let expired = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":32,
            "method":"tools/call",
            "params":{
                "name":"mcp_result",
                "arguments":{"result_ref":result_ref,"cursor":initial_cursor}
            }
        }),
    )
    .await;
    assert_eq!(
        expired["result"]["structuredContent"]["detail_status"],
        json!("detail_unavailable")
    );
    assert_eq!(
        expired["result"]["structuredContent"]["retry_original"],
        json!(false)
    );
}

#[tokio::test]
async fn issue_1733_ac_001_through_ac_004_large_source_code_uses_string_continuation() {
    use crate::routes::mcp_protocol::result_delivery::{
        deliver_oversized_result, read_continuation, CompletedOperation, ContinuationCursor,
        MAX_INLINE_CHARS,
    };
    use sha2::{Digest, Sha256};

    let (state, _) = test_api_state_with_database_url().await;
    let actor =
        domain::ActorContext::root(uuid::Uuid::now_v7(), state.bootstrap_workspace_id, "root");
    let delivery_dependencies = mcp_result_delivery_dependencies(state.as_ref());
    let source_code = "界".repeat(17_416);
    let source_sha256 = format!(
        "{:x}",
        base16ct::HexDisplay(&Sha256::digest(source_code.as_bytes()))
    );
    let delivered = deliver_oversized_result(
        &delivery_dependencies,
        &actor,
        CompletedOperation::Read {
            operation_id: "get_frontstage_block_node_code",
        },
        json!({
            "page_id": "019f51cf-4423-7ff0-93aa-6b0b1b8020bf",
            "code_ref": "frontstage-js-block-a3f0ffb2-ee70-4d48-88ac-23f0f810c0d7-code",
            "source_code": &source_code,
            "source_sha256": &source_sha256
        }),
    )
    .await;
    let compact = &delivered["structuredContent"];
    assert_eq!(compact["outcome"], json!("succeeded"));
    assert_eq!(compact["detail"]["status"], json!("continuation_available"));
    assert_eq!(compact["retry_original"], json!(false));
    let result_ref = uuid::Uuid::parse_str(
        compact["detail"]["result_ref"]
            .as_str()
            .expect("large source must expose result_ref"),
    )
    .unwrap();

    let mut cursor = ContinuationCursor::default();
    let mut reconstructed = String::new();
    for _ in 0..16 {
        let page = read_continuation(
            &delivery_dependencies,
            &actor,
            result_ref,
            cursor,
            MAX_INLINE_CHARS,
        )
        .await;
        let detail = &page["structuredContent"];
        assert_eq!(detail["detail_status"], json!("available"), "{detail}");
        assert_eq!(detail["retry_original"], json!(false));
        assert!(serde_json::to_string(detail).unwrap().chars().count() <= MAX_INLINE_CHARS);
        for entry in detail["entries"].as_array().unwrap() {
            if entry["path"] == json!("/source_code") {
                assert_eq!(entry["value_type"], json!("string_chunk"));
                assert_eq!(entry["char_offset"], json!(reconstructed.chars().count()));
                reconstructed.push_str(entry["value"].as_str().unwrap());
            }
        }
        let Some(next_cursor) = detail["next_cursor"].as_str() else {
            break;
        };
        cursor = ContinuationCursor::parse(next_cursor).expect("server cursor must be valid");
    }
    assert_eq!(reconstructed.chars().count(), 17_416);
    assert_eq!(reconstructed, source_code);
    assert_eq!(
        format!(
            "{:x}",
            base16ct::HexDisplay(&Sha256::digest(reconstructed.as_bytes()))
        ),
        source_sha256
    );

    let invalid = read_continuation(
        &delivery_dependencies,
        &actor,
        result_ref,
        ContinuationCursor::parse("v2:0:999999").unwrap(),
        MAX_INLINE_CHARS,
    )
    .await;
    assert_eq!(
        invalid["structuredContent"]["detail_status"],
        json!("invalid_cursor")
    );
    assert_eq!(invalid["structuredContent"]["retry_original"], json!(false));
}

#[tokio::test]
async fn root_1569_ac_007_ac_009_oversized_write_returns_durable_receipt_without_retry() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    let token = create_api_key(&app, &cookie, &csrf).await;
    create_interface_tool_and_binding(
        &app,
        &cookie,
        &csrf,
        "create_instance_once",
        "create_mcp_instance",
        json!({
            "mappings": [
                {"interface_param":"instance_id","mcp_param":"instance_id","required":true},
                {"interface_param":"name","mcp_param":"name","required":true},
                {"interface_param":"description_short","mcp_param":"description_short","required":true},
                {"interface_param":"status","mcp_param":"status","required":true},
                {"interface_param":"default_entry_path","mcp_param":"default_entry_path","required":true}
            ]
        }),
    )
    .await;

    let invalid_budget = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":39,
            "method":"tools/call",
            "params":{
                "name":"mcp_call",
                "arguments":{
                    "tool_id":"create_instance_once",
                    "arguments":{
                        "instance_id":"must_not_exist",
                        "name":"Must not exist",
                        "description_short":null,
                        "status":"draft",
                        "default_entry_path":"/"
                    },
                    "max_inline_chars":0
                }
            }
        }),
    )
    .await;
    assert_eq!(invalid_budget["error"]["code"], json!(-32602));
    let invalid_write_count: i64 = sqlx::query_scalar(
        "select count(*) from mcp_instances where workspace_id = $1 and instance_id = 'must_not_exist'",
    )
    .bind(state.bootstrap_workspace_id)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(invalid_write_count, 0);

    let completed = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":40,
            "method":"tools/call",
            "params":{
                "name":"mcp_call",
                "arguments":{
                    "tool_id":"create_instance_once",
                    "arguments":{
                        "instance_id":"created_once",
                        "name":"Created once",
                        "description_short":"Single dispatch fixture",
                        "status":"draft",
                        "default_entry_path":"/"
                    },
                    "max_inline_chars":1
                }
            }
        }),
    )
    .await;
    let compact = &completed["result"]["structuredContent"];
    assert_eq!(compact["outcome"], json!("succeeded"));
    assert_eq!(compact["operation_id"], json!("create_mcp_instance"));
    assert_eq!(compact["receipt_status"], json!("available"));
    assert_eq!(compact["retry_original"], json!(false));
    let receipt_id = compact["receipt_id"].as_str().unwrap();
    let initial_cursor = compact["detail"]["next_cursor"]
        .as_str()
        .expect("continuation receipt should expose the first opaque cursor");

    let created_count: i64 = sqlx::query_scalar(
        "select count(*) from mcp_instances where workspace_id = $1 and instance_id = 'created_once'",
    )
    .bind(state.bootstrap_workspace_id)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(
        created_count, 1,
        "the gateway must dispatch a write only once"
    );
    let receipt_count: i64 = sqlx::query_scalar("select count(*) from audit_logs where id = $1")
        .bind(uuid::Uuid::parse_str(receipt_id).unwrap())
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    assert_eq!(receipt_count, 1);

    let cache_key = format!("mcp-result:{}:{}", state.bootstrap_workspace_id, receipt_id);
    state
        .infrastructure
        .cache_store()
        .delete(&cache_key)
        .await
        .unwrap();
    let expired = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":41,
            "method":"tools/call",
            "params":{
                "name":"mcp_result",
                "arguments":{"result_ref":receipt_id,"cursor":initial_cursor}
            }
        }),
    )
    .await;
    let expired = &expired["result"]["structuredContent"];
    assert_eq!(expired["detail_status"], json!("detail_unavailable"));
    assert_eq!(expired["receipt"]["outcome"], json!("succeeded"));
    assert_eq!(expired["retry_original"], json!(false));
}

#[tokio::test]
async fn root_1569_ac_010_large_or_base64_like_detail_is_never_cached_or_inlined() {
    use crate::routes::mcp_protocol::result_delivery::{
        deliver_oversized_result, CompletedOperation,
    };

    let (state, _) = test_api_state_with_database_url().await;
    let actor =
        domain::ActorContext::root(uuid::Uuid::now_v7(), state.bootstrap_workspace_id, "root");
    let delivery_dependencies = mcp_result_delivery_dependencies(state.as_ref());
    for (detail, expected_reason) in [
        (
            // Beyond the chunked-continuation ceiling (16 chunks * 512 KiB):
            // each "界" serializes to 3 bytes, so this is ~9 MiB.
            json!({"rows": "界".repeat(3 * control_plane::ports::EPHEMERAL_VALUE_MAX_BYTES)}),
            "cache_capacity_exceeded",
        ),
        (
            json!({"content_base64": "A".repeat(4096)}),
            "binary_or_base64_content",
        ),
    ] {
        let delivered = deliver_oversized_result(
            &delivery_dependencies,
            &actor,
            CompletedOperation::Read {
                operation_id: "large_read_fixture",
            },
            detail,
        )
        .await;
        assert_eq!(
            delivered["structuredContent"]["detail"]["status"],
            json!("detail_unavailable")
        );
        assert_eq!(
            delivered["structuredContent"]["detail"]["reason"],
            json!(expected_reason)
        );
        assert_eq!(
            delivered["structuredContent"]["retry_original"],
            json!(false)
        );
        assert!(delivered.to_string().len() < 4_000);
    }
}

#[tokio::test]
async fn root_1569_ac_003_ac_007_ac_009_bundle_import_uses_domain_summary_and_durable_receipt() {
    use crate::routes::mcp_protocol::result_delivery::{
        deliver_oversized_result, exceeds_inline_limit, read_continuation, CompletedOperation,
        DEFAULT_INLINE_CHARS,
    };

    let (state, _) = test_api_state_with_database_url().await;
    let actor_user_id: uuid::Uuid =
        sqlx::query_scalar("select id from users where account = 'root'")
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    let actor = domain::ActorContext::root(actor_user_id, state.bootstrap_workspace_id, "root");
    let delivery_dependencies = mcp_result_delivery_dependencies(state.as_ref());
    let item_reports = (0..300)
        .map(|index| {
            json!({
                "id": format!("bundle_tool_{index}"),
                "effect": "already_present",
                "result": "already_present",
                "reason": null
            })
        })
        .collect::<Vec<_>>();
    let report = json!({
        "manifest": {
            "schema_version": "1flowbase.mcp.bundle/v2",
            "organization": "1flowbase",
            "bundle_id": "1flowbase_zh_hans",
            "bundle_version": "1.2.3",
            "locale": "zh_Hans",
            "minimum_host_version": "0.1.0",
            "exported_from_system_version": "0.1.0",
            "exported_at": "2026-08-03T00:00:00Z",
            "files": []
        },
        "current_system_version": "0.1.0",
        "version_status": "same_system_version",
        "status": "applied",
        "effect_summary": {
            "changes": 7,
            "already_present": 3,
            "conflicts": 0,
            "unavailable": 0,
            "failed": 0
        },
        "tools": item_reports,
        "instances": [],
        "connections": []
    });
    assert!(exceeds_inline_limit(&report, DEFAULT_INLINE_CHARS));
    let mut already_applied_report = report.clone();
    already_applied_report["status"] = json!("already_applied");
    already_applied_report["effect_summary"] = json!({
        "changes": 0,
        "already_present": 310,
        "conflicts": 0,
        "unavailable": 0,
        "failed": 0
    });

    let delivered = deliver_oversized_result(
        &delivery_dependencies,
        &actor,
        CompletedOperation::Write {
            operation_id: "import_mcp_bundle_library_release",
        },
        report,
    )
    .await;
    let compact = &delivered["structuredContent"];
    assert_eq!(compact["outcome"], json!("succeeded"));
    assert_eq!(
        compact["operation_id"],
        json!("import_mcp_bundle_library_release")
    );
    assert_eq!(
        compact["summary"]["bundle"],
        json!({
            "organization":"1flowbase",
            "bundle_id":"1flowbase_zh_hans",
            "bundle_version":"1.2.3",
            "locale":"zh_Hans"
        })
    );
    assert_eq!(compact["summary"]["status"], json!("applied"));
    assert_eq!(compact["summary"]["effect_summary"]["changes"], json!(7));
    assert_eq!(compact["retry_original"], json!(false));
    let receipt_id = uuid::Uuid::parse_str(compact["receipt_id"].as_str().unwrap()).unwrap();

    state
        .infrastructure
        .cache_store()
        .delete(&format!(
            "mcp-result:{}:{}",
            state.bootstrap_workspace_id, receipt_id
        ))
        .await
        .unwrap();
    let expired = read_continuation(
        &delivery_dependencies,
        &actor,
        receipt_id,
        Default::default(),
        DEFAULT_INLINE_CHARS,
    )
    .await;
    assert_eq!(
        expired["structuredContent"]["detail_status"],
        json!("detail_unavailable")
    );
    assert_eq!(
        expired["structuredContent"]["receipt"]["summary"]["bundle"]["bundle_id"],
        json!("1flowbase_zh_hans")
    );
    assert_eq!(
        expired["structuredContent"]["receipt"]["outcome"],
        json!("succeeded")
    );
    assert_eq!(expired["structuredContent"]["retry_original"], json!(false));

    let already_applied = deliver_oversized_result(
        &delivery_dependencies,
        &actor,
        CompletedOperation::Write {
            operation_id: "import_mcp_bundle_library_release",
        },
        already_applied_report,
    )
    .await;
    assert_eq!(
        already_applied["structuredContent"]["summary"]["status"],
        json!("already_applied")
    );
    assert_eq!(
        already_applied["structuredContent"]["summary"]["effect_summary"]["changes"],
        json!(0)
    );
    assert_eq!(
        already_applied["structuredContent"]["retry_original"],
        json!(false)
    );
}
