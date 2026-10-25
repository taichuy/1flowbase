use super::*;

#[tokio::test]
async fn issue_2176_cached_views_inherit_defaults_and_do_not_reexecute() {
    let base_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let pool = postgres_test_support::PostgresTestSchema::create(&base_url)
        .await
        .unwrap()
        .connect()
        .await
        .unwrap();
    storage_durable_postgres::run_migrations(&pool)
        .await
        .unwrap();
    let store = storage_durable_postgres::PgControlPlaneStore::new(pool);
    let cache = Arc::new(storage_ephemeral::MokaCacheStore::new(
        "mcp-return-test",
        1024,
    ));
    let dependencies = McpResultDeliveryDependencies::new(store, cache.clone());
    let actor = ActorContext::root(Uuid::now_v7(), Uuid::now_v7(), "root");
    let now = time::OffsetDateTime::now_utc();
    let tool = domain::McpToolRecord {
        id: Uuid::now_v7(),
        workspace_id: actor.current_workspace_id,
        tool_id: "read".into(),
        name: "Read".into(),
        short_description: String::new(),
        full_description: String::new(),
        execution_target: domain::McpToolExecutionTarget::InterfaceWrapper {
            interface_id: "read".into(),
        },
        parameter_schema: json!({}),
        result_schema: json!({}),
        input_mapping: json!({}),
        output_mapping: json!({}),
        max_inline_chars: Some(1600),
        response_fields: Some(vec!["/title".into()]),
        permission_code: None,
        risk_level: domain::McpRiskLevel::Low,
        des_id: "12345678".into(),
        des_id_required: false,
        status: domain::McpToolStatus::Enabled,
        revision: 1,
        managed_by: None,
        created_by: actor.user_id,
        updated_by: actor.user_id,
        created_at: now,
        updated_at: now,
    };
    assert_eq!(tool_inline_limit(&json!({}), &tool), Ok(1600));
    assert_eq!(
        tool_inline_limit(&json!({"max_inline_chars": 80000}), &tool),
        Ok(80000)
    );
    let source = "甲😀乙丙丁".repeat(5000);
    let selection = ResultSelection::parse(&json!({}), tool.response_fields.as_deref()).unwrap();
    let delivered = deliver_result(
        &dependencies,
        &actor,
        CompletedOperation::Read {
            operation_id: "read",
        },
        json!({"title": "summary", "body": source, "items": [{"name": "one", "other": "two"}]}),
        1600,
        &selection,
        &tool,
    )
    .await;
    let first = &delivered["structuredContent"];
    assert!(serialized(first).chars().count() <= 1600);
    assert_eq!(
        first["entries"],
        json!([{"path": "/title", "value": "summary"}])
    );
    let result_ref = Uuid::parse_str(first["detail"]["result_ref"].as_str().unwrap()).unwrap();
    let default_page = read_result(&dependencies, &actor, result_ref, &json!({})).await;
    assert_eq!(
        default_page["structuredContent"]["entries"],
        first["entries"]
    );
    let mut page = read_result(
        &dependencies,
        &actor,
        result_ref,
        &json!({
            "response_fields": ["/body"], "string_ranges": {"/body": {"offset": 3, "length": 5000}}
        }),
    )
    .await;
    let original_cursor = page["structuredContent"]["next_cursor"]
        .as_str()
        .unwrap()
        .to_owned();
    let changed = read_result(
        &dependencies,
        &actor,
        result_ref,
        &json!({"cursor": original_cursor, "response_fields": ["/title"]}),
    )
    .await;
    assert_eq!(
        changed["structuredContent"]["detail_status"],
        "invalid_cursor"
    );
    let mut reconstructed = String::new();
    loop {
        let content = &page["structuredContent"];
        assert!(serialized(content).chars().count() <= 1600);
        assert_eq!(content["detail_status"], "available");
        for entry in content["entries"].as_array().unwrap() {
            assert_eq!(entry["path"], "/body");
            assert_eq!(entry["char_offset"], 3 + reconstructed.chars().count());
            assert_eq!(entry["total_chars"], 25000);
            reconstructed.push_str(entry["value"].as_str().unwrap());
            assert_eq!(entry["next_offset"], 3 + reconstructed.chars().count());
        }
        let Some(cursor) = content["next_cursor"].as_str() else {
            break;
        };
        page = read_result(
            &dependencies,
            &actor,
            result_ref,
            &json!({"cursor": cursor}),
        )
        .await;
    }
    assert_eq!(
        reconstructed,
        source.chars().skip(3).take(5000).collect::<String>()
    );
    let empty = read_result(
        &dependencies,
        &actor,
        result_ref,
        &json!({"response_fields": []}),
    )
    .await;
    assert_eq!(empty["structuredContent"]["entries"], json!([]));
    let large = read_result(
        &dependencies,
        &actor,
        result_ref,
        &json!({"response_fields": ["/body"], "max_inline_chars": 80000}),
    )
    .await;
    assert_eq!(large["structuredContent"]["entries"][0]["value"], source);
    let nested = read_result(
        &dependencies,
        &actor,
        result_ref,
        &json!({"response_fields": ["/items/0/name"]}),
    )
    .await;
    assert_eq!(
        nested["structuredContent"]["entries"],
        json!([{"path": "/items/0/name", "value": "one"}])
    );
    let unavailable = deliver_result(
        &dependencies,
        &actor,
        CompletedOperation::Read {
            operation_id: "read",
        },
        json!({"title": "summary", "base64": "A".repeat(2048)}),
        1600,
        &selection,
        &tool,
    )
    .await;
    assert_eq!(
        unavailable["structuredContent"]["detail"]["status"],
        "detail_unavailable"
    );
    assert_eq!(
        unavailable["structuredContent"]["entries"],
        first["entries"]
    );
    assert_eq!(unavailable["structuredContent"]["next_cursor"], Value::Null);
    let wrong_actor = ActorContext::root(actor.user_id, Uuid::now_v7(), "other");
    let denied = read_result(&dependencies, &wrong_actor, result_ref, &json!({})).await;
    assert_eq!(
        denied["structuredContent"]["detail_status"],
        "detail_unavailable"
    );
    cache
        .delete(&cache_key(actor.current_workspace_id, result_ref))
        .await
        .unwrap();
    let expired = read_result(&dependencies, &actor, result_ref, &json!({})).await;
    assert_eq!(
        expired["structuredContent"]["detail_status"],
        "detail_unavailable"
    );
    assert_eq!(expired["structuredContent"]["retry_original"], false);
}

#[test]
fn json_string_split_preserves_greedy_boundaries_and_borrows_input() {
    fn serialized_reference(value: &str, max_bytes: usize) -> Option<Vec<String>> {
        let mut chunks = Vec::new();
        let mut current = String::new();
        for character in value.chars() {
            let scalar = character.to_string();
            if serialized(&json!(scalar)).len() > max_bytes {
                return None;
            }
            let mut candidate = current.clone();
            candidate.push(character);
            if serialized(&json!(candidate)).len() > max_bytes && !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
            }
            current.push(character);
        }
        if !current.is_empty() {
            chunks.push(current);
        }
        Some(chunks)
    }

    let controls: String = (0u8..=31).map(char::from).collect();
    let fixtures = [
        String::new(),
        "abcdefghi".to_owned(),
        "界😀é/\u{7f}\u{2028}\u{2029}\u{10ffff}".to_owned(),
        "\\\"\u{08}\t\n\u{0c}\r".to_owned(),
        controls,
        "a界\n😀\u{0000}b\\\"/".repeat(3),
        format!("{}界{}", "a".repeat(9), "b".repeat(9)),
        "\\\"\n\r\t\u{0000}界".repeat(20),
    ];
    for value in &fixtures {
        for budget in (0..=40).chain([usize::MAX]) {
            let expected = serialized_reference(value, budget);
            let actual = split_at_json_string_bytes(value, budget);
            assert_eq!(
                actual,
                expected
                    .as_ref()
                    .map(|chunks| chunks.iter().map(String::as_str).collect::<Vec<_>>()),
                "input={value:?}, budget={budget}"
            );
            if let Some(chunks) = actual {
                assert_eq!(chunks.concat(), *value);
                let mut offset = 0;
                for chunk in chunks {
                    assert_eq!(chunk.as_ptr(), value[offset..].as_ptr());
                    assert!(serialized(&json!(chunk)).len() <= budget);
                    offset += chunk.len();
                }
            }
        }
    }
    assert_eq!(split_at_json_string_bytes("", 0), Some(vec![]));
    assert_eq!(split_at_json_string_bytes("a", 2), None);
    assert_eq!(
        split_at_json_string_bytes("ab界c", 6),
        Some(vec!["ab", "界c"])
    );
}

#[test]
fn inline_cache_size_matches_serialized_envelope_with_return_defaults() {
    let details = [
        Value::Null,
        json!(true),
        json!(-1.25),
        json!("界😀\n\"\\\u{0000}"),
        json!([null, false, "界", {"nested": "\t"}]),
        json!({"detail": null, "format": "chunked", "return_defaults": [1, 2]}),
    ];
    let defaults = [
        None,
        Some(Value::Null),
        Some(json!({})),
        Some(json!({"max_inline_chars": 1600, "response_fields": ["/界\n\"", "/nested"]})),
    ];
    for detail in &details {
        for defaults in &defaults {
            let mut envelope = inline_cache_value(detail);
            if let Some(defaults) = defaults {
                envelope["return_defaults"] = defaults.clone();
            }
            assert_eq!(
                inline_cache_value_bytes(&serialized(detail), defaults.as_ref()),
                serialized(&envelope).len(),
                "detail={detail:?}, defaults={defaults:?}"
            );
        }
    }

    for defaults in [
        None,
        Some(json!({"max_inline_chars": 1600, "response_fields": ["/界"]})),
    ] {
        let empty_size = inline_cache_value_bytes(&serialized(&json!("")), defaults.as_ref());
        for delta in [0, 1] {
            let detail = json!("a".repeat(DETAIL_CHUNK_MAX_BYTES - empty_size + delta));
            let mut envelope = inline_cache_value(&detail);
            if let Some(defaults) = &defaults {
                envelope["return_defaults"] = defaults.clone();
            }
            assert_eq!(serialized(&envelope).len(), DETAIL_CHUNK_MAX_BYTES + delta);
            assert_eq!(
                inline_cache_value_bytes(&serialized(&detail), defaults.as_ref()),
                DETAIL_CHUNK_MAX_BYTES + delta
            );
        }
    }
    let detail = json!("界\\\"\n".repeat((DETAIL_CHUNK_MAX_BYTES - 12) / 9));
    let envelope_bytes = serialized(&inline_cache_value(&detail)).len();
    assert!(envelope_bytes > DETAIL_CHUNK_MAX_BYTES);
    assert!(serialized(&detail).len() <= DETAIL_CHUNK_MAX_BYTES);
    assert_eq!(
        inline_cache_value_bytes(&serialized(&detail), None),
        envelope_bytes
    );
}
