use super::*;

#[test]
fn provider_invocation_input_preserves_tool_message_metadata() {
    let input = ProviderInvocationInput {
        previous_response_id: Some("resp_previous".to_string()),
        messages: vec![
            ProviderMessage {
                role: ProviderMessageRole::Assistant,
                content: String::new(),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: Some(json!([
                    {
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "lookup_order",
                            "arguments": "{\"order_id\":\"A-1\"}"
                        }
                    }
                ])),
                content_blocks: None,
            },
            ProviderMessage {
                role: ProviderMessageRole::Tool,
                content: "{\"status\":\"shipped\"}".to_string(),
                name: None,
                tool_call_id: Some("call-1".to_string()),
                is_error: Some(true),
                tool_calls: None,
                content_blocks: None,
            },
        ],
        tools: vec![json!({
            "type": "function",
            "function": { "name": "lookup_order" }
        })],
        ..ProviderInvocationInput::default()
    };

    let payload = serde_json::to_value(input).unwrap();

    assert_eq!(payload["tools"][0]["function"]["name"], "lookup_order");
    assert_eq!(payload["previous_response_id"], "resp_previous");
    assert_eq!(payload["messages"][0]["tool_calls"][0]["id"], "call-1");
    assert_eq!(payload["messages"][1]["role"], "tool");
    assert_eq!(payload["messages"][1]["tool_call_id"], "call-1");
    assert_eq!(payload["messages"][1]["is_error"], true);
}

#[test]
fn provider_invocation_input_serializes_protocol_context_envelope_without_flattening_values() {
    let input = ProviderInvocationInput {
        client_protocol_envelope: Some(ProtocolContextEnvelope {
            source_protocol: "anthropic_messages".to_string(),
            query: BTreeMap::from([(
                "preview".to_string(),
                vec!["one".to_string(), "two".to_string()],
            )]),
            headers: BTreeMap::from([
                (
                    "anthropic-version".to_string(),
                    vec!["2023-06-01".to_string()],
                ),
                (
                    "anthropic-beta".to_string(),
                    vec!["prompt-caching".to_string(), "private-beta".to_string()],
                ),
            ]),
            body: BTreeMap::from([(
                "context_management".to_string(),
                json!({"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]}),
            )]),
            ..ProtocolContextEnvelope::default()
        }),
        ..ProviderInvocationInput::default()
    };

    let payload = serde_json::to_value(input).unwrap();
    let decoded: ProviderInvocationInput = serde_json::from_value(payload.clone()).unwrap();

    assert_eq!(
        payload["client_protocol_envelope"]["source_protocol"],
        "anthropic_messages"
    );
    assert_eq!(
        payload["client_protocol_envelope"]["headers"]["anthropic-version"],
        json!(["2023-06-01"])
    );
    assert_eq!(
        decoded
            .client_protocol_envelope
            .as_ref()
            .unwrap()
            .headers
            .get("anthropic-beta")
            .cloned(),
        Some(vec![
            "prompt-caching".to_string(),
            "private-beta".to_string()
        ])
    );
    assert_eq!(
        payload["client_protocol_envelope"]["query"]["preview"],
        json!(["one", "two"])
    );
    assert_eq!(
        payload["client_protocol_envelope"]["body"]["context_management"]["edits"][0]["type"],
        "clear_thinking_20251015"
    );
}

#[test]
fn protocol_context_envelope_rejects_unknown_top_level_fields() {
    let error = serde_json::from_value::<ProtocolContextEnvelope>(json!({
        "source_protocol": "anthropic_messages",
        "query": {},
        "headers": {},
        "body": {},
        "fallback": {"must_not_become_a_second_truth": true}
    }))
    .expect_err("the protocol context shell must stay minimal and typed");

    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn source_protocol_request_serializes_authentication_presentation_without_a_secret() {
    let envelope: ProtocolContextEnvelope = serde_json::from_value(json!({
        "source_protocol": "anthropic_messages",
        "source_request": {
            "authentication": "authorization_bearer",
            "body": {
                "model": "claude-opus-4-8",
                "messages": [{
                    "role": "user",
                    "content": [{
                        "type": "text",
                        "text": "hello",
                        "cache_control": {"type": "ephemeral"}
                    }]
                }]
            }
        }
    }))
    .expect("SourceProtocolContext must deserialize the safe source request shape");

    let encoded = serde_json::to_string(&envelope).unwrap();
    assert!(encoded.contains("authorization_bearer"));
    assert!(encoded.contains("cache_control"));
    assert!(!encoded.contains("Bearer "));
    assert!(!encoded.contains("api_key"));
}

#[test]
fn anthropic_source_request_requires_the_restore_v2_provider_profile() {
    let input = ProviderInvocationInput {
        client_protocol_envelope: Some(
            serde_json::from_value(json!({
                "source_protocol": "anthropic_messages",
                "source_request": {
                    "authentication": "authorization_bearer",
                    "body": {
                        "model": "claude-opus-4-8",
                        "messages": [{"role": "user", "content": "hello"}]
                    }
                }
            }))
            .unwrap(),
        ),
        ..ProviderInvocationInput::default()
    };

    let (legacy_wire, legacy_receipt) = input
        .to_current_provider_generate_wire_value(&[
            "protocol_context.restore.anthropic_messages.v1".to_string(),
        ])
        .expect("legacy residual-only provider must degrade with a receipt");
    assert!(legacy_wire.get("client_protocol_envelope").is_none());
    assert!(legacy_receipt
        .decisions
        .contains(&ProviderGenerateTranslationDecision::OmittedProtocolContextProfileMismatch));

    let (source_wire, source_receipt) = input
        .to_current_provider_generate_wire_value(&[
            "protocol_context.restore.anthropic_messages.v2".to_string(),
        ])
        .expect("SourceProtocolContext provider must receive the complete safe context");
    assert_eq!(
        source_wire["client_protocol_envelope"]["source_request"]["authentication"],
        "authorization_bearer"
    );
    assert!(source_receipt.decisions.is_empty());
}

#[test]
fn provider_projection_rejects_credential_bearing_source_body_root() {
    let input = ProviderInvocationInput {
        client_protocol_envelope: Some(
            serde_json::from_value(json!({
                "source_protocol": "anthropic_messages",
                "source_request": {
                    "authentication": "x_api_key",
                    "body": {
                        "model": "claude-opus-4-8",
                        "messages": [],
                        "authorization": "must-not-cross"
                    }
                }
            }))
            .unwrap(),
        ),
        ..ProviderInvocationInput::default()
    };

    input
        .to_current_provider_generate_wire_value(&[
            "protocol_context.restore.anthropic_messages.v2".to_string(),
        ])
        .expect_err("credential-bearing source body roots must fail closed");
}

#[test]
fn d4_ac_002_native_responses_passthrough_has_one_manifest_capability_name() {
    let capability = ProviderInvocationCapability::ResponsesNativePassthrough;

    assert_eq!(
        capability.manifest_capability_name(),
        "responses.native_passthrough"
    );
    assert_eq!(
        serde_json::to_value(capability).unwrap(),
        json!("responses.native_passthrough")
    );
}

#[test]
fn wp_r1_generate_omits_undeclared_optional_context_with_a_bounded_receipt() {
    const CACHE_CANARY: &str = "cache-control-raw-canary";
    const USER_CANARY: &str = "end-user-raw-canary";
    let input = ProviderInvocationInput {
        protocol: "openai_compatible".to_string(),
        system: vec![NativePromptBlock::Text {
            text: CACHE_CANARY.to_string(),
            cache_control: Some(NativePromptCacheControl {
                cache_type: NativePromptCacheControlType::Ephemeral,
                ttl: None,
            }),
        }],
        request_context: NativeModelRequestContext {
            end_user_reference: Some(USER_CANARY.to_string()),
        },
        client_protocol_envelope: Some(ProtocolContextEnvelope {
            source_protocol: "anthropic_messages".to_string(),
            query: BTreeMap::from([("preview".to_string(), vec!["raw-query-canary".to_string()])]),
            ..ProtocolContextEnvelope::default()
        }),
        ..ProviderInvocationInput::default()
    };

    let (wire, receipt) = input
        .to_current_provider_generate_wire_value(&[])
        .expect("undeclared optional foreign context should degrade for Generate");

    assert_eq!(wire["system"][0]["text"], CACHE_CANARY);
    assert!(wire["system"][0].get("cache_control").is_none());
    assert!(wire.get("request_context").is_none());
    assert!(wire.get("client_protocol_envelope").is_none());
    assert_eq!(
        receipt.decisions,
        BTreeSet::from([
            ProviderGenerateTranslationDecision::OmittedSystemPromptCacheControl,
            ProviderGenerateTranslationDecision::OmittedEndUserReference,
            ProviderGenerateTranslationDecision::OmittedProtocolContextProfileMismatch,
        ])
    );
    let encoded = serde_json::to_string(&receipt).unwrap();
    assert!(encoded.len() <= 512);
    for raw in [CACHE_CANARY, USER_CANARY, "raw-query-canary"] {
        assert!(!encoded.contains(raw));
    }

    let mut provider_metadata = json!({"provider": "fixture"});
    receipt
        .attach_to_provider_metadata(&mut provider_metadata)
        .expect("receipt should share the existing provider metadata ledger");
    assert_eq!(
        provider_metadata[PROVIDER_GENERATE_TRANSLATION_RECEIPT_METADATA_KEY]["decisions"],
        json!([
            "omitted_system_prompt_cache_control",
            "omitted_end_user_reference",
            "omitted_protocol_context_profile_mismatch"
        ])
    );
    let mut conflicting_metadata = json!({
        (PROVIDER_GENERATE_TRANSLATION_RECEIPT_METADATA_KEY): {"provider_owned": true}
    });
    assert!(receipt
        .attach_to_provider_metadata(&mut conflicting_metadata)
        .is_err());
}

#[test]
fn wp_r14a_generate_retains_a_foreign_envelope_only_for_its_exact_profile() {
    let foreign_source = ProviderInvocationInput {
        protocol: "openai_chat".to_string(),
        client_protocol_envelope: Some(ProtocolContextEnvelope {
            source_protocol: "anthropic_messages".to_string(),
            body: BTreeMap::from([("future_option".to_string(), json!({"enabled": true}))]),
            ..ProtocolContextEnvelope::default()
        }),
        required_capabilities: BTreeSet::from([ProviderInvocationCapability::ProtocolContext]),
        ..ProviderInvocationInput::default()
    };
    let (wire, receipt) = foreign_source
        .to_current_provider_generate_wire_value(&[
            "protocol_context.consume.anthropic_messages.v1".to_string(),
        ])
        .expect("an exact foreign consume profile should retain the host envelope");
    assert_eq!(
        wire["client_protocol_envelope"]["source_protocol"],
        "anthropic_messages"
    );
    assert!(wire.get("required_capabilities").is_none());
    assert!(receipt.decisions.is_empty());

    let (wire, receipt) = foreign_source
        .to_current_provider_generate_wire_value(&[
            "protocol_context.consume.openai_chat.v1".to_string()
        ])
        .expect("a profile mismatch must omit ordinary residual context");
    assert!(wire.get("client_protocol_envelope").is_none());
    assert!(wire.get("required_capabilities").is_none());
    assert_eq!(
        receipt.decisions,
        BTreeSet::from([
            ProviderGenerateTranslationDecision::OmittedProtocolContextProfileMismatch,
        ])
    );
}

#[test]
fn wp_r14a_native_passthrough_capability_remains_fail_closed() {
    let native_passthrough = ProviderInvocationInput {
        required_capabilities: BTreeSet::from([
            ProviderInvocationCapability::ResponsesNativePassthrough,
        ]),
        ..ProviderInvocationInput::default()
    };
    let error = native_passthrough
        .to_current_provider_generate_wire_value(&[])
        .unwrap_err();
    assert!(error.to_string().contains("responses.native_passthrough"));
}

#[test]
fn root_1805_network_egress_handoff_is_capability_gated_and_lease_free() {
    let mut input = ProviderInvocationInput::default();
    input.set_network_egress_context(ProviderNetworkEgressContext {
        mode: ProviderNetworkEgressMode::RequiredHttpProxy,
        http_proxy_url: "http://127.0.0.1:3128".to_string(),
        expires_at: "1735689600".to_string(),
        required: true,
    });

    let error = input
        .to_current_provider_generate_wire_value(&[])
        .unwrap_err();
    assert!(error.to_string().contains("network_egress_handoff/v1"));

    let (wire, _) = input
        .to_current_provider_generate_wire_value(&["network_egress_handoff/v1".to_string()])
        .expect("a Provider that declares the handoff capability must receive its proxy context");
    assert_eq!(
        wire["run_context"]["network_egress"],
        json!({
            "mode": "required_http_proxy",
            "http_proxy_url": "http://127.0.0.1:3128",
            "expires_at": "1735689600",
            "required": true,
        })
    );
    assert_eq!(
        wire["required_capabilities"],
        json!(["network_egress_handoff/v1"])
    );
    assert!(wire["run_context"]["network_egress"]
        .get("cleanup_token")
        .is_none());
    assert!(wire["run_context"]["network_egress"]
        .get("provider_id")
        .is_none());
    assert!(wire["run_context"]["network_egress"]
        .get("pool_id")
        .is_none());
}

#[test]
fn wp_r14a_count_tokens_and_compact_share_exact_profile_projection() {
    let envelope = ProtocolContextEnvelope {
        source_protocol: "anthropic_messages".to_string(),
        query: BTreeMap::from([("preview".to_string(), vec!["one".to_string()])]),
        ..ProtocolContextEnvelope::default()
    };
    let count_tokens = ProviderCountTokensInput::from_invocation(ProviderInvocationInput {
        client_protocol_envelope: Some(envelope.clone()),
        required_capabilities: BTreeSet::from([ProviderInvocationCapability::ProtocolContext]),
        ..ProviderInvocationInput::default()
    });
    let count_wire = count_tokens
        .to_current_provider_wire_value(&[
            "count_tokens".to_string(),
            "protocol_context.restore.openai_responses.v1".to_string(),
        ])
        .expect("CountTokens profile mismatch should omit and continue");
    assert!(count_wire.get("client_protocol_envelope").is_none());
    assert!(count_wire.get("required_capabilities").is_none());
    let count_wire = count_tokens
        .to_current_provider_wire_value(&[
            "count_tokens".to_string(),
            "protocol_context.consume.anthropic_messages.v1".to_string(),
        ])
        .expect("CountTokens exact consume profile should retain the envelope");
    assert_eq!(
        count_wire["client_protocol_envelope"]["source_protocol"],
        "anthropic_messages"
    );
    assert!(count_wire.get("required_capabilities").is_none());

    let compact = ProviderInvocationInput {
        operation: ProviderWireOperation::Compact,
        profile: Some(ProviderCompactProfile::ResponsesCompact),
        client_protocol_envelope: Some(envelope),
        required_capabilities: BTreeSet::from([ProviderInvocationCapability::ProtocolContext]),
        ..ProviderInvocationInput::default()
    };
    let compact_wire = compact
        .to_current_provider_compact_wire_value(&[
            "compact.responses_compact".to_string(),
            "protocol_context.restore.anthropic_messages.v1".to_string(),
        ])
        .expect("Compact exact restore profile should retain the envelope");
    assert_eq!(
        compact_wire["client_protocol_envelope"]["source_protocol"],
        "anthropic_messages"
    );
    assert_eq!(
        compact_wire["required_capabilities"],
        json!(["compact.responses_compact"])
    );
    let compact_wire = compact
        .to_current_provider_compact_wire_value(&[
            "compact.responses_compact".to_string(),
            "protocol_context.restore.openai_chat.v1".to_string(),
        ])
        .expect("Compact profile mismatch should omit and continue");
    assert!(compact_wire.get("client_protocol_envelope").is_none());
    assert_eq!(
        compact_wire["required_capabilities"],
        json!(["compact.responses_compact"])
    );
}

#[test]
fn wp_r1_generate_rejects_protocol_envelope_collisions_and_authentication_fields() {
    for envelope in [
        ProtocolContextEnvelope {
            source_protocol: "anthropic_messages".to_string(),
            body: BTreeMap::from([("model".to_string(), json!("collision"))]),
            ..ProtocolContextEnvelope::default()
        },
        ProtocolContextEnvelope {
            source_protocol: "anthropic_messages".to_string(),
            headers: BTreeMap::from([(
                "authorization".to_string(),
                vec!["Bearer secret-canary".to_string()],
            )]),
            ..ProtocolContextEnvelope::default()
        },
        ProtocolContextEnvelope {
            source_protocol: "anthropic_messages".to_string(),
            body: BTreeMap::from([(
                "future_extension".to_string(),
                json!({"nested": {"api_key": "secret-canary"}}),
            )]),
            ..ProtocolContextEnvelope::default()
        },
    ] {
        let input = ProviderInvocationInput {
            protocol: "openai_compatible".to_string(),
            client_protocol_envelope: Some(envelope),
            ..ProviderInvocationInput::default()
        };
        assert!(input.to_current_provider_generate_wire_value(&[]).is_err());
    }
}

#[test]
fn ac_003_semantic_requirements_expose_canonical_manifest_capability_names() {
    let required = semantic_required_capabilities(
        &[NativePromptBlock::Text {
            text: "Cache this block".to_string(),
            cache_control: Some(NativePromptCacheControl {
                cache_type: NativePromptCacheControlType::Ephemeral,
                ttl: None,
            }),
        }],
        &NativeModelRequestContext {
            end_user_reference: Some("external-user-123".to_string()),
        },
    );

    assert_eq!(
        required,
        BTreeSet::from([
            ProviderInvocationCapability::SystemPromptBlocks,
            ProviderInvocationCapability::SystemPromptCacheControl,
            ProviderInvocationCapability::EndUserReference,
        ])
    );
    assert_eq!(
        required
            .iter()
            .map(|capability| capability.manifest_capability_name())
            .collect::<Vec<_>>(),
        vec![
            "system_prompt_blocks",
            "system_prompt_cache_control",
            "end_user_reference",
        ]
    );
}

#[test]
fn root_1534_ac_001_message_blocks_derive_reasoning_capabilities_and_bounded_receipt() {
    let messages = vec![ProviderMessage {
        role: ProviderMessageRole::Assistant,
        content: "visible answer".to_string(),
        name: None,
        tool_call_id: None,
        is_error: None,
        tool_calls: None,
        content_blocks: Some(json!([
            {"type": "reasoning", "text": "private reasoning", "signature": "sig"},
            {"type": "text", "text": "visible answer"},
            {"type": "reasoning_redacted", "data": "opaque"}
        ])),
    }];

    assert_eq!(
        message_block_required_capabilities(&messages).unwrap(),
        BTreeSet::from([
            ProviderInvocationCapability::MessageBlocksReasoningHistoryV1,
            ProviderInvocationCapability::MessageBlocksRedactedReasoningHistoryV1,
        ])
    );

    let input = ProviderInvocationInput {
        messages,
        ..ProviderInvocationInput::default()
    };
    let (wire, receipt) = input
        .to_current_provider_generate_wire_value(&[
            "message_blocks.reasoning_history.v1".to_string(),
            "message_blocks.redacted_reasoning_history.v1".to_string(),
        ])
        .expect("a conforming Provider owns canonical message-block lowering");

    assert_eq!(
        wire["required_capabilities"],
        json!([
            "message_blocks.reasoning_history.v1",
            "message_blocks.redacted_reasoning_history.v1"
        ])
    );
    assert_eq!(receipt.decisions.len(), 2);
    assert!(receipt
        .decisions
        .contains(&ProviderGenerateTranslationDecision::DelegatedReasoningHistoryToProvider));
    assert!(receipt.decisions.contains(
        &ProviderGenerateTranslationDecision::DelegatedRedactedReasoningHistoryToProvider
    ));
}

#[test]
fn root_1534_ac_002_unknown_or_untyped_canonical_message_blocks_fail_closed() {
    for content_blocks in [
        json!({"type": "reasoning"}),
        json!([{"text": "missing type"}]),
        json!([{"type": "future_vendor_block", "payload": "must-not-cross"}]),
    ] {
        let input = ProviderInvocationInput {
            messages: vec![ProviderMessage {
                role: ProviderMessageRole::Assistant,
                content: "visible answer".to_string(),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: None,
                content_blocks: Some(content_blocks),
            }],
            ..ProviderInvocationInput::default()
        };

        let error = input
            .to_current_provider_generate_wire_value(&[
                "message_blocks.reasoning_history.v1".to_string()
            ])
            .expect_err("unknown canonical blocks must not reach a Provider wire");
        assert!(
            error.to_string().contains("canonical block")
                || error.to_string().contains("content_blocks")
        );
    }
}

#[test]
fn root_1534_ac_003_explicit_anthropic_capabilities_accept_reasoning_block_semantics() {
    let input = ProviderInvocationInput {
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::Assistant,
            content: "visible answer".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(json!([
                {"type": "reasoning", "text": "reasoning", "signature": "sig"},
                {"type": "reasoning_redacted", "data": "opaque"},
                {"type": "text", "text": "visible answer"}
            ])),
        }],
        ..ProviderInvocationInput::default()
    };

    input
        .to_current_provider_generate_wire_value(&[
            "message_blocks.reasoning_history.v1".to_string(),
            "message_blocks.redacted_reasoning_history.v1".to_string(),
            "protocol_context.restore.anthropic_messages.v2".to_string(),
        ])
        .expect("Anthropic declares both canonical block semantics explicitly");
}

#[test]
fn root_1743_d1_ac_002_generate_projects_reasoning_and_visible_text_lossily() {
    let input = ProviderInvocationInput {
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::Assistant,
            content: "visible answer".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(json!([
                {"type": "reasoning", "text": "private reasoning"},
                {"type": "text", "text": "visible answer"}
            ])),
        }],
        ..ProviderInvocationInput::default()
    };

    let (provider_wire, receipt) = input
        .to_current_provider_generate_wire_value(&[])
        .expect("visible assistant text makes reasoning omission safe");
    assert_eq!(receipt.fidelity, Some(ProviderProjectionFidelity::Lossy));
    assert_eq!(
        receipt.loss_codes,
        BTreeSet::from([ProviderProjectionLossCode::ReasoningHistoryOmitted])
    );
    assert_eq!(
        provider_wire["messages"][0]["content_blocks"],
        json!([{"type": "text", "text": "visible answer"}])
    );
    let provenance = receipt
        .provenance
        .expect("projection must identify its immutable canonical source");
    assert_eq!(
        provenance.source,
        ProviderProjectionSource::CanonicalInvocation
    );
    assert_eq!(provenance.omitted_blocks[0].message_index, 0);
    assert_eq!(provenance.omitted_blocks[0].block_index, 0);
    assert_eq!(
        provenance.omitted_blocks[0].block_kind,
        ProviderCanonicalBlockKind::Reasoning
    );
    assert_eq!(provenance.preserved_blocks[0].block_index, 1);
}

#[test]
fn root_1743_d1_ac_003_exact_provider_preserves_reasoning_history() {
    let input = ProviderInvocationInput {
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::Assistant,
            content: "visible answer".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(json!([
                {"type": "reasoning", "text": "private reasoning"},
                {"type": "text", "text": "visible answer"}
            ])),
        }],
        ..ProviderInvocationInput::default()
    };

    let projection = input
        .project_current_provider_generate(&["reasoning_history_input_supported".to_string()])
        .expect("a synthetic exact-capable provider must receive canonical reasoning");
    assert_eq!(
        projection.receipt.fidelity,
        Some(ProviderProjectionFidelity::Exact)
    );
    assert!(projection.receipt.loss_codes.is_empty());
    assert_eq!(projection.provider_bound_input.messages, input.messages);
    assert_eq!(
        projection.provider_bound_input.required_capabilities,
        BTreeSet::from([ProviderInvocationCapability::ReasoningHistoryInputSupported])
    );
}

#[test]
fn root_1743_d1_ac_005_reasoning_only_projection_is_typed_unsupported() {
    let input = ProviderInvocationInput {
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::Assistant,
            content: String::new(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(json!([
                {"type": "reasoning", "text": "private reasoning"}
            ])),
        }],
        ..ProviderInvocationInput::default()
    };

    let error = input
        .project_current_provider_generate(&[])
        .expect_err("omitting a reasoning-only assistant message must fail closed");
    match error {
        ProviderGenerateProjectionError::Unsupported {
            code,
            block,
            receipt,
        } => {
            assert_eq!(
                code,
                ProviderProjectionErrorCode::ReasoningOnlyMessageUnsupported
            );
            assert_eq!(block.message_index, 0);
            assert_eq!(block.block_index, 0);
            assert_eq!(
                receipt.fidelity,
                Some(ProviderProjectionFidelity::Unsupported)
            );
            assert_eq!(receipt.error_code, Some(code));
            assert_eq!(receipt.provenance.unwrap().omitted_blocks[0], block);
        }
        ProviderGenerateProjectionError::InvalidContract { message } => {
            panic!("expected typed unsupported, got invalid contract: {message}")
        }
    }
}

#[test]
fn root_1743_d1_ac_004_signed_and_redacted_reasoning_never_become_text() {
    let input = ProviderInvocationInput {
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::Assistant,
            content: "visible answer".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(json!([
                {"type": "reasoning", "text": "signed", "signature": "opaque-signature"},
                {"type": "reasoning_redacted", "data": "opaque-redaction"},
                {"type": "text", "text": "visible answer"}
            ])),
        }],
        ..ProviderInvocationInput::default()
    };

    let projection = input.project_current_provider_generate(&[]).unwrap();
    let encoded = serde_json::to_string(&projection.provider_bound_input).unwrap();
    assert!(!encoded.contains("signed"));
    assert!(!encoded.contains("opaque-signature"));
    assert!(!encoded.contains("opaque-redaction"));
    assert_eq!(
        projection.receipt.loss_codes,
        BTreeSet::from([
            ProviderProjectionLossCode::RedactedReasoningOmitted,
            ProviderProjectionLossCode::SignedReasoningOmitted,
        ])
    );
}

#[test]
fn root_1743_generic_history_exactness_does_not_cover_redacted_reasoning() {
    let input = ProviderInvocationInput {
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::Assistant,
            content: "visible answer".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(json!([
                {"type": "reasoning", "text": "ordinary"},
                {"type": "reasoning_redacted", "data": "opaque"},
                {"type": "text", "text": "visible answer"}
            ])),
        }],
        ..ProviderInvocationInput::default()
    };

    let projection = input
        .project_current_provider_generate(&["reasoning_history_input_supported".to_string()])
        .expect("ordinary reasoning remains exact while redacted reasoning degrades safely");
    assert_eq!(
        projection.receipt.fidelity,
        Some(ProviderProjectionFidelity::Lossy)
    );
    assert_eq!(
        projection.provider_bound_input.messages[0].content_blocks,
        Some(json!([
            {"type": "reasoning", "text": "ordinary"},
            {"type": "text", "text": "visible answer"}
        ]))
    );
    assert_eq!(
        projection.provider_bound_input.required_capabilities,
        BTreeSet::from([ProviderInvocationCapability::ReasoningHistoryInputSupported])
    );
    assert_eq!(
        projection.receipt.loss_codes,
        BTreeSet::from([ProviderProjectionLossCode::RedactedReasoningOmitted])
    );
}

#[test]
fn root_1743_projection_receipt_bounds_large_history_locator_summary() {
    let content_blocks = (0..256)
        .flat_map(|index| {
            [
                json!({"type": "reasoning", "text": format!("private-{index}")}),
                json!({"type": "text", "text": format!("visible-{index}")}),
            ]
        })
        .collect::<Vec<_>>();
    let input = ProviderInvocationInput {
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::Assistant,
            content: "visible answer".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(json!(content_blocks)),
        }],
        ..ProviderInvocationInput::default()
    };

    let projection = input.project_current_provider_generate(&[]).unwrap();
    let provenance = projection.receipt.provenance.as_ref().unwrap();
    assert_eq!(provenance.preserved_block_count, 256);
    assert_eq!(provenance.omitted_block_count, 256);
    assert!(provenance.capped);
    assert_eq!(provenance.preserved_blocks.len(), 16);
    assert_eq!(provenance.omitted_blocks.len(), 16);
    let receipt = serde_json::to_string(&projection.receipt).unwrap();
    assert!(receipt.len() < 5_000);
    assert!(!receipt.contains("private-255"));
    assert!(!receipt.contains("visible-255"));
}

#[test]
fn root_1743_reasoning_capabilities_round_trip_with_manifest_names() {
    for (capability, name) in [
        (
            ProviderInvocationCapability::ReasoningOutputSupported,
            "reasoning_output_supported",
        ),
        (
            ProviderInvocationCapability::ReasoningHistoryInputSupported,
            "reasoning_history_input_supported",
        ),
        (
            ProviderInvocationCapability::NativeContinuationSupported,
            "native_continuation_supported",
        ),
    ] {
        assert_eq!(capability.manifest_capability_name(), name);
        let value = serde_json::to_value(capability).unwrap();
        assert_eq!(value, json!(name));
        assert_eq!(
            serde_json::from_value::<ProviderInvocationCapability>(value).unwrap(),
            capability
        );
    }
}

#[test]
fn c1_count_tokens_wire_is_tagged_and_requires_a_declared_capability() {
    let input = ProviderCountTokensInput::from_invocation(ProviderInvocationInput {
        contract_version: Default::default(),
        provider_instance_id: "provider-1".to_string(),
        provider_code: "anthropic".to_string(),
        protocol: "anthropic_messages".to_string(),
        model: "claude-fixture".to_string(),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "count this exact prompt".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: None,
        }],
        ..ProviderInvocationInput::default()
    });

    let wire = input
        .to_current_provider_wire_value(&["count_tokens".to_string()])
        .expect("declared CountTokens capability should serialize the current typed operation");
    assert_eq!(wire["operation"], json!("count_tokens"));
    assert_eq!(wire["contract_version"], json!("1flowbase.provider/v2"));
    assert_eq!(
        wire["messages"][0]["content"],
        json!("count this exact prompt")
    );
    assert_eq!(wire["tools"], json!([]));

    let result = serde_json::to_value(ProviderCountTokensResult {
        operation: ProviderWireOperation::CountTokens,
        input_tokens: 37,
        ..ProviderCountTokensResult::default()
    })
    .expect("typed CountTokens result should serialize");
    assert_eq!(
        result,
        json!({
            "operation": "count_tokens",
            "input_tokens": 37,
            "method": "upstream_api",
            "coverage": "complete",
            "unknown_block_count": 0
        })
    );

    assert!(matches!(
        input.to_current_provider_wire_value(&[]),
        Err(ProviderCountTokensError::Unsupported { capabilities })
            if capabilities == vec!["count_tokens"]
    ));
}

#[test]
fn k2_compact_wire_derives_the_selected_profile_capability_and_closed_result_shape() {
    let input = ProviderInvocationInput {
        operation: ProviderWireOperation::Compact,
        profile: Some(ProviderCompactProfile::ResponsesCompactionV2),
        provider_instance_id: "provider-1".to_string(),
        provider_code: "openai".to_string(),
        protocol: "openai_responses".to_string(),
        model: "gpt-5.4-mini".to_string(),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "retain this turn".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: None,
        }],
        ..ProviderInvocationInput::default()
    };

    let wire = input
        .to_current_provider_compact_wire_value(&["compact.responses_compaction_v2".to_string()])
        .expect("the selected Compact profile should require its exact manifest row");
    assert_eq!(wire["operation"], json!("compact"));
    assert_eq!(wire["profile"], json!("responses_compaction_v2"));
    assert_eq!(
        wire["required_capabilities"],
        json!(["compact.responses_compaction_v2"])
    );

    let result = serde_json::to_value(ProviderCompactResult::CompletedOpaqueCompactionItem {
        operation: ProviderWireOperation::Compact,
        profile: ProviderCompactProfile::ResponsesCompactionV2,
        response_id: Some("resp_compact".to_string()),
        compaction_item: json!({
            "type": "compaction",
            "encrypted_content": "opaque-byte-canary"
        }),
        encrypted_content: "opaque-byte-canary".to_string(),
    })
    .expect("the V2 Compact result should have one closed typed representation");
    assert_eq!(
        result,
        json!({
            "result_type": "completed_opaque_compaction_item",
            "operation": "compact",
            "profile": "responses_compaction_v2",
            "response_id": "resp_compact",
            "compaction_item": {
                "type": "compaction",
                "encrypted_content": "opaque-byte-canary"
            },
            "encrypted_content": "opaque-byte-canary"
        })
    );

    assert!(matches!(
        input.to_current_provider_compact_wire_value(&[]),
        Err(ProviderCompactError::Unsupported {
            profile: ProviderCompactProfile::ResponsesCompactionV2,
            capabilities,
        }) if capabilities == vec!["compact.responses_compaction_v2"]
    ));
}

#[test]
fn wp_r1_compact_keeps_optional_context_capabilities_strict() {
    let input = ProviderInvocationInput {
        operation: ProviderWireOperation::Compact,
        profile: Some(ProviderCompactProfile::ResponsesCompact),
        system: vec![NativePromptBlock::Text {
            text: "Cache this compact block".to_string(),
            cache_control: Some(NativePromptCacheControl {
                cache_type: NativePromptCacheControlType::Ephemeral,
                ttl: None,
            }),
        }],
        ..ProviderInvocationInput::default()
    };

    assert!(matches!(
        input.to_current_provider_compact_wire_value(&[
            "compact.responses_compact".to_string()
        ]),
        Err(ProviderCompactError::Unsupported { capabilities, .. })
            if capabilities.contains(&"system_prompt_cache_control")
    ));
}

#[test]
fn ac_002_current_generate_input_requires_explicit_contract_version() {
    let error = serde_json::from_value::<ProviderInvocationInput>(json!({
        "provider_instance_id": "provider-1",
        "provider_code": "anthropic",
        "protocol": "anthropic_messages",
        "model": "claude-fable-5",
        "messages": [{ "role": "user", "content": "hello" }],
        "system": []
    }))
    .unwrap_err();

    assert!(error.to_string().contains("contract_version"));
}

#[test]
fn ac_002_current_generate_input_strictly_rejects_legacy_and_unknown_fields() {
    let legacy_contract = serde_json::from_value::<ProviderInvocationInput>(json!({
        "contract_version": "1flowbase.provider/v1",
        "provider_instance_id": "provider-1",
        "provider_code": "legacy",
        "protocol": "openai_compatible",
        "model": "legacy-model",
        "messages": [],
        "system": "legacy system projection"
    }))
    .unwrap_err();
    assert!(legacy_contract
        .to_string()
        .contains("1flowbase.provider/v1"));

    let unknown_field = serde_json::from_value::<ProviderInvocationInput>(json!({
        "contract_version": "1flowbase.provider/v2",
        "provider_instance_id": "provider-1",
        "provider_code": "current",
        "protocol": "openai_compatible",
        "model": "current-model",
        "messages": [],
        "system": [],
        "raw_body": "must-not-be-accepted"
    }))
    .unwrap_err();
    assert!(unknown_field.to_string().contains("raw_body"));
}

#[test]
fn ac_005_wire_audit_is_bounded_and_excludes_raw_or_sensitive_values() {
    const SECRET_CANARY: &str = "secret-canary-do-not-audit";
    const PROMPT_CANARY: &str = "prompt-canary-do-not-audit";
    const RAW_CANARY: &str = "raw-body-canary-do-not-audit";

    let input = ProviderInvocationInput {
        provider_config: json!({
            "api_key": SECRET_CANARY,
            "raw_body": RAW_CANARY,
        }),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: PROMPT_CANARY.to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: None,
        }],
        system: vec![NativePromptBlock::text(PROMPT_CANARY)],
        trace_context: BTreeMap::from([("secret-header".to_string(), SECRET_CANARY.to_string())]),
        run_context: BTreeMap::from([("raw_body".to_string(), json!(RAW_CANARY))]),
        ..ProviderInvocationInput::default()
    };

    let audit = input.wire_audit();
    let payload = serde_json::to_value(&audit).unwrap();
    let encoded = serde_json::to_string(&audit).unwrap();

    assert_eq!(payload["operation"], "generate");
    assert_eq!(payload["message_count"], 1);
    assert_eq!(payload["system_block_count"], 1);
    assert_eq!(payload["trace_context_entry_count"], 1);
    assert_eq!(payload["run_context_entry_count"], 1);
    assert!(!payload["counts_capped"].as_bool().unwrap());
    assert!(encoded.len() <= 1024, "wire audit must remain bounded");
    assert!(!encoded.contains(SECRET_CANARY));
    assert!(!encoded.contains(PROMPT_CANARY));
    assert!(!encoded.contains(RAW_CANARY));
    assert!(!encoded.contains("api_key"));
    assert!(!encoded.contains("raw_body"));
}

#[test]
fn d4_ac_016_native_transport_serializes_for_provider_but_debug_stays_redacted() {
    const SECRET: &str = "Bearer native-transport-secret";
    let input = ProviderInvocationInput {
        native_transport: Some(ProviderNativeTransport {
            protocol: "openai_responses".to_string(),
            wire_body: json!({
                "tools": [{"type": "mcp", "authorization": SECRET}],
                "future_extension": {"preserve": true}
            }),
            digest: "sha256:test".to_string(),
            size_bytes: 128,
        }),
        ..ProviderInvocationInput::default()
    };

    let wire = serde_json::to_value(&input).expect("provider invocation should serialize");
    assert_eq!(
        wire["native_transport"]["wire_body"]["future_extension"]["preserve"],
        true
    );
    assert_eq!(
        wire["native_transport"]["wire_body"]["tools"][0]["authorization"],
        SECRET
    );
    assert!(input.wire_audit().has_native_transport);
    assert!(!format!("{input:?}").contains(SECRET));
    assert!(!serde_json::to_string(&input.wire_audit())
        .expect("wire audit should serialize")
        .contains(SECRET));
}

