use serde_json::json;

use super::*;
use crate::application_public_api::model_catalog::{AgentModelCapabilities, AgentModelReasoning};

#[test]
fn extracts_start_node_model_list_from_strings_and_objects() {
    let document = json!({
        "graph": {
            "nodes": [
                {
                    "id": "node-start",
                    "type": "start",
                    "config": {
                        "model_list": [
                            {
                                "id": "qwen3.6-35b-a3b",
                                "name": "Qwen 3.6 35B",
                                "context_window": 128000,
                                "auto_compact_token_limit": 110000
                            },
                            "deepseek-v4-flash",
                            {"id": "deepseek-v4-flash", "name": "Duplicate"}
                        ]
                    }
                }
            ]
        }
    });

    assert_eq!(
        extract_model_list_from_start_node(&document),
        vec![
            OpenAiCompatibleModel {
                id: "qwen3.6-35b-a3b".into(),
                name: Some("Qwen 3.6 35B".into()),
                context_window: Some(128000),
                max_context_window: None,
                max_output_tokens: None,
                auto_compact_token_limit: Some(110000),
                capabilities: AgentModelCapabilities::default(),
                reasoning: None,
            },
            OpenAiCompatibleModel {
                id: "deepseek-v4-flash".into(),
                name: None,
                context_window: None,
                max_context_window: None,
                max_output_tokens: None,
                auto_compact_token_limit: None,
                capabilities: AgentModelCapabilities::default(),
                reasoning: None,
            },
        ]
    );
}

#[test]
fn extracts_default_model_when_start_node_has_no_model_list() {
    let document = json!({
        "graph": {
            "nodes": [
                {
                    "id": "node-start",
                    "type": "start",
                    "config": {
                        "input_fields": []
                    }
                }
            ]
        }
    });

    assert_eq!(
        extract_model_list_from_start_node(&document),
        vec![OpenAiCompatibleModel {
            id: "1flowbase".into(),
            name: Some("1flowbase".into()),
            context_window: Some(128000),
            max_context_window: Some(128000),
            max_output_tokens: Some(8000),
            auto_compact_token_limit: Some(108800),
            capabilities: AgentModelCapabilities {
                reasoning: true,
                tool_call: true,
                multimodal: true,
                structured_output: true,
            },
            reasoning: Some(AgentModelReasoning {
                default_effort: Some("medium".into()),
                supported_efforts: vec![
                    "minimal".into(),
                    "low".into(),
                    "medium".into(),
                    "high".into(),
                    "xhigh".into(),
                ],
            }),
        }]
    );
}

#[test]
fn ac_001_chat_tools_map_to_native_inputs() {
    let translated = translate_chat_completion_request(json!({
        "model": "deepseek-v4-flash",
        "messages": [
            { "role": "user", "content": "say hello" }
        ],
        "tools": [
            {
                "type": "function",
                "function": {
                    "name": "read_file",
                    "description": "Read a file",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "file_path": { "type": "string" }
                        }
                    }
                }
            }
        ],
        "tool_choice": "auto"
    }))
    .expect("Chat tools should map to Native inputs");

    assert_eq!(
        translated.request.inputs.as_value()["tools"][0]["name"],
        "read_file"
    );
    assert_eq!(
        translated.request.inputs.as_value()["tool_choice"]["type"],
        "auto"
    );
}

#[test]
fn ac_001_chat_callback_tool_history_maps_to_native() {
    let external_tool_call_id = "calltask_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa_call_weather_lookup";

    let translated = translate_chat_completion_request(json!({
        "model": "deepseek-v4-flash",
        "messages": [
            { "role": "user", "content": "first question" },
            {
                "role": "assistant",
                "content": null,
                "tool_calls": [
                    {
                        "id": external_tool_call_id,
                        "type": "function",
                        "function": {
                            "name": "lookup_weather",
                            "arguments": "{}"
                        }
                    }
                ]
            },
            {
                "role": "tool",
                "tool_call_id": external_tool_call_id,
                "content": "{\"temperature\":21}"
            },
            { "role": "assistant", "content": "old answer" },
            { "role": "user", "content": "next question" }
        ]
    }))
    .expect("callback history should map to Native");
    assert_eq!(
        translated.request.history[1]["tool_calls"][0]["id"],
        "call_weather_lookup"
    );
    assert_eq!(
        translated.request.history[2]["tool_call_id"],
        "call_weather_lookup"
    );
}

#[test]
fn ac_001_chat_provider_native_tool_ids_are_preserved() {
    let translated = translate_chat_completion_request(json!({
        "model": "deepseek-v4-flash",
        "messages": [
            { "role": "user", "content": "first question" },
            {
                "role": "assistant",
                "content": null,
                "tool_calls": [
                    {
                        "id": "calltask_not-a-valid-callback",
                        "type": "function",
                        "function": {
                            "name": "lookup_weather",
                            "arguments": "{}"
                        }
                    }
                ]
            },
            {
                "role": "tool",
                "tool_call_id": "provider_native_call",
                "content": "{\"temperature\":21}"
            },
            { "role": "user", "content": "next question" }
        ]
    }))
    .expect("provider-native tool ids should be preserved");
    assert_eq!(
        translated.request.history[1]["tool_calls"][0]["id"],
        "calltask_not-a-valid-callback"
    );
    assert_eq!(
        translated.request.history[2]["tool_call_id"],
        "provider_native_call"
    );
}

#[test]
fn d2_ac_007_legacy_function_call_has_an_unsupported_receipt() {
    let error = translate_chat_completion_request(json!({
        "model": "deepseek-v4-flash",
        "messages": [
            { "role": "user", "content": "say hello" }
        ],
        "function_call": { "name": "read_file" }
    }))
    .expect_err("function_call has no D2 canonical owner");

    assert_eq!(error.param.as_deref(), Some("function_call"));
    assert!(error
        .report
        .has_decision("$.function_call", TranslationDecisionKind::Unsupported));
}

#[test]
fn maps_responses_text_input_into_native_run() {
    let request = map_response_request(
        json!({
            "model": "deepseek-v4-flash",
            "input": "Summarize the incident",
            "user": "external-user-1",
            "metadata": {"trace_id": "trace-responses"},
            "stream": true
        }),
        None,
    )
    .unwrap();

    assert_eq!(request.query, "Summarize the incident");
    assert_eq!(request.model.as_deref(), Some("deepseek-v4-flash"));
    assert_eq!(request.response_mode.as_deref(), Some("streaming"));
    assert_eq!(request.conversation["user"], json!("external-user-1"));
    assert_eq!(request.metadata.trace_id(), Some("trace-responses"));
}

#[test]
fn codex_store_false_is_a_dropped_no_storage_hint() {
    let translated = translate_response_request(json!({
        "model": "1flowbase",
        "input": "hello",
        "store": false
    }))
    .expect("store=false should not require OpenAI server-side storage");

    assert!(translated
        .report
        .has_decision("$.store", TranslationDecisionKind::Dropped));
}

#[test]
fn codex_parallel_tool_calls_is_an_optional_provider_scheduling_hint() {
    for value in [false, true] {
        let translated = translate_response_request(json!({
            "model": "1flowbase",
            "input": "hello",
            "parallel_tool_calls": value
        }))
        .expect("parallel tool scheduling must not bind a request to an OpenAI Provider");

        assert!(translated
            .report
            .has_decision("$.parallel_tool_calls", TranslationDecisionKind::Dropped));
    }
}

#[test]
fn ac_016_reasoning_encrypted_content_include_is_an_optional_native_hint() {
    let mut translated = translate_response_request(json!({
        "model": "1flowbase",
        "input": "hello",
        "include": ["reasoning.encrypted_content"]
    }))
    .expect("encrypted reasoning include should not bind a request to an OpenAI Provider");

    assert!(translated
        .report
        .has_decision("$.include", TranslationDecisionKind::Dropped));
    assert!(translated
        .request
        .metadata
        .take_provider_transport_payload()
        .is_none());
}

#[test]
fn codex_null_reasoning_is_an_absent_optional_parameter() {
    let translated = translate_response_request(json!({
        "model": "1flowbase",
        "input": "hello",
        "reasoning": null
    }))
    .expect("Codex null reasoning should be equivalent to an absent optional parameter");

    assert!(translated
        .report
        .has_decision("$.reasoning", TranslationDecisionKind::Dropped));
    assert!(translated
        .request
        .execution
        .model_parameters()
        .and_then(|parameters| parameters.reasoning())
        .is_none());
}

#[test]
fn codex_cache_and_client_metadata_are_typed_optional_hints() {
    let translated = translate_response_request(json!({
        "model": "1flowbase",
        "input": "hello",
        "prompt_cache_key": "thread-1",
        "client_metadata": {
            "session_id": "session-1",
            "thread_id": "thread-1"
        }
    }))
    .expect("Codex cache and diagnostic metadata are optional Native hints");

    assert!(translated
        .report
        .has_decision("$.prompt_cache_key", TranslationDecisionKind::Dropped));
    assert!(translated
        .report
        .has_decision("$.client_metadata", TranslationDecisionKind::Dropped));
}

#[test]
fn codex_metadata_hints_retain_their_wire_types() {
    for (field, value) in [
        ("prompt_cache_key", json!(42)),
        ("client_metadata", json!({"session_id": 42})),
    ] {
        let mut request = json!({"model": "1flowbase", "input": "hello"});
        request[field] = value;
        let error = translate_response_request(request)
            .expect_err("Codex metadata hint wire types must remain explicit");
        assert_eq!(error.param.as_deref(), Some(field));
        assert!(error
            .report
            .has_decision(&format!("$.{field}"), TranslationDecisionKind::Rejected));
    }
}

#[test]
fn d4_ac_016_unknown_include_remains_exact_in_native_transport() {
    let mut translated = translate_response_request(json!({
        "model": "1flowbase",
        "input": "hello",
        "include": ["message.output_text"]
    }))
    .expect("unknown include projections should remain in native Responses transport");

    assert!(translated
        .report
        .has_decision("$.include", TranslationDecisionKind::Exact));
    let payload = translated
        .request
        .metadata
        .take_provider_transport_payload()
        .expect("include should remain in ephemeral provider transport");
    assert_eq!(payload.wire_body()["include"][0], "message.output_text");
}

#[test]
fn responses_include_requires_an_array_wire_type() {
    let error = translate_response_request(json!({
        "model": "1flowbase",
        "input": "hello",
        "include": "reasoning.encrypted_content"
    }))
    .expect_err("include must retain its array wire type");

    assert_eq!(error.param.as_deref(), Some("include"));
    assert!(error
        .report
        .has_decision("$.include", TranslationDecisionKind::Rejected));
}

#[test]
fn responses_parallel_tool_calls_requires_a_boolean() {
    let error = translate_response_request(json!({
        "model": "1flowbase",
        "input": "hello",
        "parallel_tool_calls": "false"
    }))
    .expect_err("parallel_tool_calls must retain its boolean wire type");

    assert_eq!(error.param.as_deref(), Some("parallel_tool_calls"));
    assert_eq!(error.code, "invalid_request");
    assert!(error
        .report
        .has_decision("$.parallel_tool_calls", TranslationDecisionKind::Rejected));
}

#[test]
fn opencode_chat_stream_options_include_usage_is_a_dropped_hint() {
    let translated = translate_chat_completion_request(json!({
        "model": "1flowbase",
        "stream": true,
        "stream_options": { "include_usage": true },
        "messages": [{ "role": "user", "content": "hello" }]
    }))
    .expect("compatible streaming already projects usage when available");

    assert!(translated
        .report
        .has_decision("$.stream_options", TranslationDecisionKind::Dropped));
}

#[test]
fn stale_chat_tool_output_escapes_nul_before_native_history() {
    let translated = translate_chat_completion_request(json!({
        "model": "1flowbase",
        "messages": [
            { "role": "user", "content": "run command" },
            {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_shell",
                    "type": "function",
                    "function": { "name": "shell", "arguments": "{}" }
                }]
            },
            { "role": "tool", "tool_call_id": "call_shell", "content": "STDERR:\n\0after" },
            { "role": "user", "content": "continue" }
        ]
    }))
    .expect("NUL tool history should remain representable in PostgreSQL JSON");

    assert_eq!(
        translated.request.history[2]["content"],
        json!("STDERR:\n\\u0000after")
    );
}

#[test]
fn ac_002_responses_tools_map_to_native_inputs() {
    let translated = translate_response_request(json!({
        "model": "1flowbase",
        "input": "hi",
        "tools": [
            {
                "type": "function",
                "name": "shell",
                "description": "Run a command",
                "parameters": {
                    "type": "object",
                    "properties": { "command": { "type": "array" } }
                },
                "strict": false
            }
        ]
    }))
    .expect("Responses tools should map to Native inputs");
    assert_eq!(
        translated.request.inputs.as_value()["tools"][0]["name"],
        "shell"
    );
    assert_eq!(
        translated
            .request
            .metadata
            .responses_transport_requirement(),
        crate::application_public_api::native::ResponsesTransportRequirement::SemanticCompatible
    );
}

#[test]
fn d4_ac_001_responses_classifier_marks_opaque_tools_choices_items_and_hints_native() {
    for request in [
        json!({
            "model": "1flowbase",
            "input": "hi",
            "tools": [{"type": "web_search_preview"}],
            "tool_choice": "required"
        }),
        json!({
            "model": "1flowbase",
            "input": "hi",
            "tool_choice": {"type": "hosted_tool", "name": "search"}
        }),
        json!({
            "model": "1flowbase",
            "input": [{"type": "item_reference", "id": "item_1"}]
        }),
        json!({
            "model": "1flowbase",
            "input": "hi",
            "store": true
        }),
        json!({
            "model": "1flowbase",
            "input": "hi",
            "truncation": "auto"
        }),
        json!({
            "model": "1flowbase",
            "input": "hi",
            "text": {"format": {"type": "json_schema"}}
        }),
    ] {
        assert_eq!(
            responses_transport_requirement(
                request.as_object().expect("fixture is a Responses object")
            ),
            crate::application_public_api::native::ResponsesTransportRequirement::NativePassthrough
        );
    }
}

#[test]
fn ac_017_codex_optional_hosted_tools_stay_in_context_without_binding_transport() {
    let mut translated = translate_response_request(json!({
        "model": "1flowbase",
        "input": [{"type": "message", "role": "user", "content": "inspect git"}],
        "tools": [
            {
                "type": "function",
                "name": "exec_command",
                "description": "Run a command",
                "parameters": {"type": "object"},
                "strict": false
            },
            {"type": "namespace", "name": "multi_agent_v1"},
            {"type": "web_search"}
        ],
        "tool_choice": "auto",
        "parallel_tool_calls": false,
        "store": false,
        "include": []
    }))
    .expect("optional Codex hosted tools should not bind a cross-provider request");

    assert_eq!(
        translated
            .request
            .metadata
            .responses_transport_requirement(),
        crate::application_public_api::native::ResponsesTransportRequirement::SemanticCompatible
    );
    assert_eq!(
        translated.request.inputs.as_value()["tools"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        translated.request.inputs.as_value()["tools"][0]["name"],
        "exec_command"
    );
    assert!(translated
        .request
        .metadata
        .take_provider_transport_payload()
        .is_none());
    let envelope = translated
        .request
        .client_protocol_envelope
        .expect("omitted tools remain available to a matching Provider profile");
    assert_eq!(
        envelope.body[OPENAI_RESPONSES_OPTIONAL_TOOLS_CONTEXT_FIELD]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        envelope.body[OPENAI_RESPONSES_OPTIONAL_TOOLS_CONTEXT_FIELD][0]["type"],
        "namespace"
    );
    assert_eq!(
        envelope.body[OPENAI_RESPONSES_OPTIONAL_TOOLS_CONTEXT_FIELD][1]["type"],
        "web_search"
    );
    assert!(translated
        .report
        .has_decision("$.tools[1]", TranslationDecisionKind::Exact));
    assert!(translated
        .report
        .has_decision("$.tools[2]", TranslationDecisionKind::Exact));
}

#[test]
fn ac_017_explicit_hosted_tool_choice_remains_native_passthrough() {
    let translated = translate_response_request(json!({
        "model": "1flowbase",
        "input": "search",
        "tools": [{"type": "web_search"}],
        "tool_choice": {"type": "web_search"}
    }))
    .expect("an explicitly selected hosted tool remains a native operation");

    assert_eq!(
        translated
            .request
            .metadata
            .responses_transport_requirement(),
        crate::application_public_api::native::ResponsesTransportRequirement::NativePassthrough
    );
}

#[test]
fn ac_017_explicit_function_choice_must_select_a_projected_function() {
    let error = translate_response_request(json!({
        "model": "1flowbase",
        "input": "delegate",
        "tools": [
            {"type": "function", "name": "exec_command", "parameters": {"type": "object"}},
            {"type": "namespace", "name": "multi_agent_v1"}
        ],
        "tool_choice": {"type": "function", "name": "multi_agent_v1"}
    }))
    .expect_err("an omitted namespace cannot be selected as a Native function");

    assert_eq!(error.param.as_deref(), Some("tool_choice"));
}

#[test]
fn ac_016_safe_unknown_responses_extension_stays_optional_protocol_context() {
    let mut translated = translate_response_request(json!({
        "model": "1flowbase",
        "input": "hi",
        "future_responses_extension": {"opaque": true}
    }))
    .expect("safe unknown Responses fields should remain optional protocol context");

    assert_eq!(
        translated
            .request
            .metadata
            .responses_transport_requirement(),
        crate::application_public_api::native::ResponsesTransportRequirement::SemanticCompatible
    );
    assert!(translated
        .request
        .metadata
        .take_provider_transport_payload()
        .is_none());
    assert_eq!(
        translated.request.client_protocol_envelope.unwrap().body["future_responses_extension"]
            ["opaque"],
        true
    );
}

#[test]
fn d4_ac_016_native_responses_translation_retains_real_wire_payload_only_in_sidecar() {
    const SECRET: &str = "Bearer transport-secret-canary";
    let mut translated = translate_response_request(json!({
        "model": "1flowbase",
        "input": "hi",
        "tools": [{
            "type": "mcp",
            "server_url": "https://mcp.example.test",
            "authorization": SECRET
        }],
        "future_responses_extension": {"opaque": true}
    }))
    .expect("native Responses request should retain its provider wire body");

    let payload = translated
        .request
        .metadata
        .take_provider_transport_payload()
        .expect("native request should carry an ephemeral transport sidecar");
    let summary = translated
        .request
        .metadata
        .provider_transport_summary_value()
        .expect("durable metadata should retain only a transport summary");
    assert_eq!(summary["protocol"], "openai_responses");
    assert_eq!(summary["storage"], "ephemeral");
    assert!(summary["digest"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:")));
    assert!(!summary.to_string().contains(SECRET));
    assert_eq!(
        payload.wire_body()["future_responses_extension"]["opaque"],
        true
    );
    assert_eq!(payload.wire_body()["tools"][0]["authorization"], SECRET);
    assert!(!format!("{payload:?}").contains(SECRET));
    assert!(!serde_json::to_string(&translated.request)
        .expect("Native request should serialize")
        .contains(SECRET));
}

#[test]
fn d4_ac_016_native_responses_keeps_opaque_input_item_without_fabricating_history() {
    let mut translated = translate_response_request(json!({
        "model": "1flowbase",
        "input": [{"type": "item_reference", "id": "item_1"}]
    }))
    .expect("native Responses input item should bypass semantic reconstruction");

    assert!(translated.request.query.is_empty());
    assert!(translated.request.history.is_empty());
    let payload = translated
        .request
        .metadata
        .take_provider_transport_payload()
        .expect("opaque input should remain in ephemeral transport");
    assert_eq!(payload.wire_body()["input"][0]["id"], "item_1");
}

#[test]
fn d5_ac_004_hosted_tools_stay_out_of_gateway_tool_execution_inputs() {
    let mut translated = translate_response_request(json!({
        "model": "1flowbase",
        "input": "search",
        "tools": [
            {"type": "web_search", "external_web_access": false},
            {"type": "code_interpreter", "container": {"type": "auto"}},
            {"type": "image_generation", "quality": "high"}
        ]
    }))
    .expect("hosted tools should use native Responses transport");

    assert!(translated.request.inputs.as_value().get("tools").is_none());
    assert!(translated.request.history.is_empty());
    let payload = translated
        .request
        .metadata
        .take_provider_transport_payload()
        .expect("hosted tools should remain in ephemeral provider transport");
    assert_eq!(payload.wire_body()["tools"][0]["type"], "web_search");
    assert_eq!(payload.wire_body()["tools"][1]["type"], "code_interpreter");
    assert_eq!(payload.wire_body()["tools"][2]["type"], "image_generation");
}

#[test]
fn d6_ac_003_orphan_mcp_approval_response_is_rejected_before_run_creation() {
    let error = translate_response_request(json!({
        "model": "1flowbase",
        "input": [{
            "type": "mcp_approval_response",
            "approval_request_id": "approval_provider_owned",
            "approve": true
        }]
    }))
    .expect_err("MCP approval response must name a provider continuation");

    assert_eq!(error.code, "invalid_request");
    assert_eq!(error.param.as_deref(), Some("previous_response_id"));
}

#[test]
fn d6_ac_001_mcp_approval_response_remains_opaque_with_provider_continuation() {
    let mut translated = translate_response_request_with_context_and_previous(
        json!({
            "model": "1flowbase",
            "previous_response_id": "resp_provider_owned",
            "input": [{
                "type": "mcp_approval_response",
                "approval_request_id": "approval_provider_owned",
                "approve": false,
                "future_extension": {"opaque": true}
            }]
        }),
        OpenAiResponsesRequestContext::responses(),
        Some(OpenAiPreviousResponseContext {
            response_id: "resp_provider_owned".to_string(),
            external_user: None,
            external_conversation_id: None,
            answer: None,
        }),
    )
    .expect("MCP approval response should continue through the native provider lane");

    assert!(translated.request.history.is_empty());
    let payload = translated
        .request
        .metadata
        .take_provider_transport_payload()
        .expect("MCP approval response should remain ephemeral");
    assert_eq!(
        payload.wire_body()["input"][0]["approval_request_id"],
        "approval_provider_owned"
    );
    assert_eq!(payload.wire_body()["input"][0]["approve"], false);
    assert_eq!(
        payload.wire_body()["input"][0]["future_extension"]["opaque"],
        true
    );
}

#[test]
fn ac_002_responses_function_calls_map_to_native_history() {
    let translated = translate_response_request(json!({
                "model": "1flowbase",
                "input": [
                    {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "查代码"}]},
                    {"type": "function_call", "call_id": "call_a", "name": "shell", "arguments": "{}"},
                    {"type": "function_call", "call_id": "call_b", "name": "shell", "arguments": "{}"},
                    {"type": "function_call_output", "call_id": "call_a", "output": "a-result"},
                    {"type": "function_call_output", "call_id": "call_b", "output": "b-result"},
                    {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "继续"}]}
                ]
            }))
        .expect("Responses function calls should map to Native history");
    assert_eq!(
        translated.request.history[1]["tool_calls"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(translated.request.history[2]["tool_call_id"], "call_a");
}

#[test]
fn ac_008_responses_reconstructable_tool_output_can_start_a_new_turn() {
    let translated = translate_response_request(json!({
            "model": "1flowbase",
            "input": [
                {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "查库存"}]},
                {"type": "function_call", "call_id": "call_inventory", "name": "lookup", "arguments": "{}"},
                {"type": "function_call_output", "call_id": "call_inventory", "output": "7"}
            ]
        }))
        .expect("paired Responses tool history should start a new turn without invented input");

    assert_eq!(translated.request.query, "");
    assert_eq!(
        translated.request.history[1]["tool_calls"][0]["id"],
        "call_inventory"
    );
    assert_eq!(
        translated.request.history[2]["tool_call_id"],
        "call_inventory"
    );
}

#[test]
fn ac_008_responses_orphan_tool_output_is_rejected() {
    let error = translate_response_request(json!({
        "model": "1flowbase",
        "input": [
            {"type": "function_call_output", "call_id": "call_orphan", "output": "7"}
        ]
    }))
    .expect_err("orphan Responses tool output must not invent a function call");

    assert_eq!(error.code, "invalid_request");
    assert!(error.message.contains("matching function_call"));
}

#[test]
fn ac_003_native_responses_replay_preserves_opaque_item_identity_without_semantic_history() {
    let mut translated = translate_response_request(json!({
                "model": "1flowbase",
                "input": [
                    {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "看图"}]},
                    {"type": "reasoning", "id": "rs_1", "summary": [], "content": [{"type": "reasoning_text", "text": "想一想"}], "encrypted_content": null},
                    {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "先查目录"}]},
                    {"type": "function_call", "id": "fc_1", "call_id": "call_shell_1", "name": "shell", "arguments": "{\"command\":[\"ls\"]}"},
                    {"type": "function_call_output", "call_id": "call_shell_1", "output": "uploads\nweb"},
                    {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "继续找导航栏代码"}]}
                ]
            }))
        .expect("opaque Responses replay items should use native provider transport");

    assert!(translated
        .request
        .history
        .iter()
        .all(|item| item.get("reasoning").is_none()));
    let payload = translated
        .request
        .metadata
        .take_provider_transport_payload()
        .expect("opaque replay identity should remain in ephemeral provider transport");
    assert_eq!(payload.wire_body()["input"][1]["id"], "rs_1");
    assert_eq!(payload.wire_body()["input"][3]["id"], "fc_1");
}

#[test]
fn ac_002_previous_response_id_is_accepted_for_route_context_resolution() {
    let translated = translate_response_request(json!({
        "model": "deepseek-v4-flash",
        "input": [{"role": "user", "content": [{"type": "input_text", "text": "Continue"}]}],
        "previous_response_id": "resp_11111111-1111-1111-1111-111111111111"
    }))
    .expect("previous_response_id should be accepted");
    assert_eq!(translated.request.query, "Continue");
}
