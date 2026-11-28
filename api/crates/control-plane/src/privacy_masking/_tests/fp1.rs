use super::*;

fn custom(id: &str, pattern: &str) -> PrivacyMaskingRuleDto {
    PrivacyMaskingRuleDto {
        rule_id: id.into(),
        name: id.into(),
        kind: PrivacyMaskingRuleKind::CustomRegex,
        builtin_code: None,
        placeholder_kind: id.to_ascii_uppercase(),
        pattern: Some(pattern.into()),
        enabled: true,
        priority: 0,
    }
}

fn binding(
    kind: PrivacyMaskingSubjectKind,
    id: Uuid,
    rules: &[&str],
) -> PrivacyMaskingSubjectBindingDto {
    PrivacyMaskingSubjectBindingDto {
        binding_id: id.to_string(),
        subject_kind: kind,
        subject_id: id.to_string(),
        subject_name: String::new(),
        enabled: true,
        restore_output: true,
        rule_mode: PrivacyMaskingRuleMode::SelectedRules,
        rule_ids: rules.iter().map(|id| (*id).into()).collect(),
    }
}

fn context() -> PrivacyInvocationContext {
    PrivacyInvocationContext {
        workspace_id: Uuid::new_v4(),
        flow_run_id: None,
        application_id: Some(Uuid::new_v4()),
        provider_instance_id: Uuid::new_v4(),
        provider_instance_name: "fixture".into(),
        model_id: "fixture".into(),
        protocol: "openai_responses".into(),
    }
}

#[tokio::test]
async fn audit_snippet_must_not_retain_plaintext_of_later_stage() {
    let (repo, kernel) = test_kernel();
    let ctx = context();
    let later = "SYNTHETIC_LONG_VALUE_NEVER_PERSIST_ANY_PART_OF_THIS";
    let dto = PrivacyMaskingWorkspacePolicyDto {
        enabled: true,
        allowlist: vec![],
        rules: vec![
            custom("first", "FIRST_SECRET_[0-9]+"),
            custom("later", later),
            custom("wrap", r"WRAP<[^>]+>"),
        ],
        subject_bindings: vec![
            binding(
                PrivacyMaskingSubjectKind::AgentFlow,
                ctx.application_id.unwrap(),
                &["first"],
            ),
            binding(
                PrivacyMaskingSubjectKind::ModelProviderInstance,
                ctx.provider_instance_id,
                &["later", "wrap"],
            ),
        ],
    };
    kernel
        .sync_workspace_policy(Uuid::new_v4(), ctx.workspace_id, None, &dto)
        .await
        .unwrap();
    let mut input = ProviderInvocationInput {
        system: vec![NativePromptBlock::text(format!(
            "FIRST_SECRET_1 {later}; WRAP<FIRST_SECRET_2>"
        ))],
        ..Default::default()
    };
    let prepared = kernel
        .prepare_invocation(&ctx, &mut input)
        .await
        .unwrap()
        .unwrap();
    kernel.finalize_invocation(&prepared, None).await;
    let logs = repo.logs.lock().unwrap();
    assert_eq!(logs.len(), 2);
    let final_text = input.system[0].text_content();
    for entry in logs.iter().flat_map(|log| &log.matched_rules) {
        assert!(entry.prefix_hint.is_empty());
        assert!(!entry.masked_snippet.contains("SYNTHETIC"));
        assert!(!entry.masked_snippet.contains("FIRST_SECRET"));
        if final_text.contains(&entry.placeholder) {
            assert!(final_text.contains(entry.masked_snippet.trim_matches('…')));
        } else {
            assert_eq!(entry.masked_snippet, entry.placeholder);
        }
    }
}

#[tokio::test]
async fn semantic_provider_tool_payload_must_be_masked() {
    let (_, kernel) = test_kernel();
    let ctx = context();
    let secret = "SYNTHETIC_SECRET_812";
    let dto = PrivacyMaskingWorkspacePolicyDto {
        enabled: true,
        allowlist: vec![],
        rules: vec![custom("secret", secret)],
        subject_bindings: vec![binding(
            PrivacyMaskingSubjectKind::ModelProviderInstance,
            ctx.provider_instance_id,
            &["secret"],
        )],
    };
    kernel
        .sync_workspace_policy(Uuid::new_v4(), ctx.workspace_id, None, &dto)
        .await
        .unwrap();
    let mut input = ProviderInvocationInput {
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: secret.into(),
            name: None,
            tool_call_id: None,
            is_error: None,
            content_blocks: Some(
                json!([{"type":"text","text":secret,"metadata":{"id":secret,"role":secret}}]),
            ),
            tool_calls: Some(
                json!([{"id":secret,"function":{"name":"lookup","arguments":{"id":secret,"type":secret}}}]),
            ),
        }],
        tools: vec![
            json!({"type":"function","function":{"name":"lookup","description":secret,"parameters":{"properties":{"id":{"default":secret},"type":{"enum":[secret]}}}}}),
        ],
        response_format: Some(
            json!({"type":"json_schema","json_schema":{"description":secret,"schema":{"properties":{"id":{"const":secret}}}}}),
        ),
        native_transport: Some(ProviderNativeTransport {
            protocol: "openai_responses".into(),
            wire_body: json!({"model":secret,"previous_response_id":secret,"input":[{"id":secret,"type":"function_call","call_id":secret,"name":secret,"arguments":json!({"id":secret,"model":secret}).to_string()}],"metadata":{"id":secret}}),
            digest: String::new(),
            size_bytes: 0,
        }),
        ..Default::default()
    };
    kernel
        .prepare_invocation(&ctx, &mut input)
        .await
        .unwrap()
        .unwrap();
    assert!(!input.messages[0].content.contains(secret));
    let blocks = input.messages[0].content_blocks.as_ref().unwrap();
    assert!(!blocks.to_string().contains(secret));
    let tool_calls = input.messages[0].tool_calls.as_ref().unwrap();
    assert_eq!(tool_calls[0]["id"], secret);
    assert!(!tool_calls[0]["function"]["arguments"]
        .to_string()
        .contains(secret));
    assert!(!json!(input.tools).to_string().contains(secret));
    assert!(!json!(input.response_format).to_string().contains(secret));
    let wire = input.native_transport.unwrap().wire_body;
    for value in [
        &wire["model"],
        &wire["previous_response_id"],
        &wire["input"][0]["id"],
        &wire["input"][0]["call_id"],
        &wire["input"][0]["name"],
    ] {
        assert_eq!(*value, secret);
    }
    let arguments: serde_json::Value =
        serde_json::from_str(wire["input"][0]["arguments"].as_str().unwrap()).unwrap();
    assert!(arguments["id"].as_str().unwrap().starts_with("{{SECRET_"));
    assert_eq!(arguments["id"], arguments["model"]);
    assert!(!wire["metadata"].to_string().contains(secret));
}

#[tokio::test]
async fn invalidation_blocks_stale_cache_refill() {
    for global in [false, true] {
        let (repo, kernel) = test_kernel();
        let ctx = context();
        let install = Uuid::new_v4();
        let dto = PrivacyMaskingWorkspacePolicyDto {
            enabled: true,
            allowlist: vec![],
            rules: vec![custom("secret", "SECRET")],
            subject_bindings: vec![binding(
                PrivacyMaskingSubjectKind::ModelProviderInstance,
                ctx.provider_instance_id,
                &["secret"],
            )],
        };
        kernel
            .sync_workspace_policy(install, ctx.workspace_id, None, &dto)
            .await
            .unwrap();
        let captured = Arc::new(tokio::sync::Barrier::new(2));
        let resume = Arc::new(tokio::sync::Barrier::new(2));
        *repo.next_read_fence.lock().unwrap() = Some((captured.clone(), resume.clone()));
        let ws = ctx.workspace_id;
        let stale_reader = kernel.has_active_policies(ws);
        let invalidate = async {
            captured.wait().await;
            repo.delete_privacy_masking_policies_for_installation(install)
                .await
                .unwrap();
            if global {
                kernel.invalidate_installation(install);
            } else {
                kernel.invalidate_workspace(ws);
            }
            resume.wait().await;
        };
        let (observed, _) = tokio::join!(stale_reader, invalidate);
        assert!(
            !observed.unwrap(),
            "in-flight reader must retry after invalidation"
        );
        assert!(
            !kernel.has_active_policies(ws).await.unwrap(),
            "stale policies must not be cached"
        );
    }
}

#[tokio::test]
async fn dense_existing_placeholders_linear_scale() {
    let (_, kernel) = test_kernel();
    let dto = PrivacyMaskingWorkspacePolicyDto {
        enabled: true,
        allowlist: vec![],
        subject_bindings: vec![],
        rules: vec![custom("secret", r"SECRET[0-9]+|\{\{KEEP_abcdefgh\}\}")],
    };
    let text = (0..8000)
        .map(|i| format!("{{{{KEEP_abcdefgh}}}} SECRET{i} "))
        .collect::<String>();
    let result = kernel
        .preview_masking(Uuid::new_v4(), &text, Some(&dto), true)
        .await
        .unwrap();
    assert_eq!(result.total_hits, 8000);
    assert_eq!(
        result.masked_text.matches("{{KEEP_abcdefgh}}").count(),
        8000
    );
    assert_eq!(result.restored_text.as_deref(), Some(text.as_str()));
}

const INNER: &str = "{{INNER_abcdefgh}}";
const OUTER: &str = "{{OUTER_bcdefghi}}";
const RAW: &str = "引号\"\\\n\r\t中文";

fn nested_restorer() -> ProviderStreamEventRestorer {
    let stack = RestorationStack::new(
        vec![
            RestorationStage {
                stage_index: 0,
                map: HashMap::from([(INNER.into(), RAW.into())]),
            },
            RestorationStage {
                stage_index: 1,
                map: HashMap::from([(OUTER.into(), format!("<{}>", INNER))]),
            },
        ],
        HashMap::new(),
    );
    ProviderStreamEventRestorer::new(stack)
}

#[test]
fn native_delta_restores_placeholder_across_events() {
    let mut restorer = nested_restorer();
    let mut text = String::new();
    let mut arguments = String::new();
    let mut thinking = String::new();
    for (i, fragment) in OUTER.as_bytes().chunks(3).enumerate() {
        let fragment = std::str::from_utf8(fragment).unwrap();
        let arg = format!(
            "{}{fragment}{}",
            if i == 0 { r#"{"id":""# } else { "" },
            if (i + 1) * 3 >= OUTER.len() {
                r#""}"#
            } else {
                ""
            }
        );
        for (protocol, event) in [
            (
                "openai_responses",
                json!({"type":"response.output_text.delta","item_id":"a","output_index":0,"content_index":0,"delta":fragment}),
            ),
            (
                "openai_responses",
                json!({"type":"response.function_call_arguments.delta","item_id":"b","output_index":1,"delta":arg}),
            ),
            (
                "anthropic_messages",
                json!({"type":"content_block_delta","index":1,"delta":{"type":"thinking_delta","thinking":fragment}}),
            ),
        ] {
            for event in restorer.process_event(ProviderStreamEvent::NativeEvent {
                protocol: protocol.into(),
                event,
            }) {
                let ProviderStreamEvent::NativeEvent { event, .. } = event else {
                    panic!("not native")
                };
                match event["type"].as_str().unwrap() {
                    "response.output_text.delta" => text.push_str(event["delta"].as_str().unwrap()),
                    "response.function_call_arguments.delta" => {
                        arguments.push_str(event["delta"].as_str().unwrap())
                    }
                    _ => thinking.push_str(event["delta"]["thinking"].as_str().unwrap()),
                }
            }
        }
    }
    assert_eq!(text, format!("<{RAW}>"));
    assert_eq!(thinking, text);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&arguments).unwrap()["id"],
        text
    );
}

#[test]
fn tool_call_argument_stream_preserves_json_escaping() {
    let mut restorer = nested_restorer();
    let raw_json = format!(r#"{{"key":"{OUTER}","untouched":"\\n"}}"#);
    let mut joined = String::new();
    for fragment in raw_json.as_bytes().chunks(3) {
        for event in restorer.process_event(ProviderStreamEvent::ToolCallDelta {
            call_id: "tool".into(),
            delta: json!(std::str::from_utf8(fragment).unwrap()),
        }) {
            let ProviderStreamEvent::ToolCallDelta { delta, .. } = event else {
                panic!("not a delta")
            };
            joined.push_str(delta.as_str().unwrap());
        }
    }
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&joined).unwrap(),
        json!({"key":format!("<{RAW}>"),"untouched":"\\n"})
    );
}

#[tokio::test]
async fn durable_input_rewrite_failure_blocks_provider_preparation() {
    let (repo, kernel) = test_kernel();
    let mut ctx = context();
    ctx.flow_run_id = Some(Uuid::new_v4());
    let mut strict = binding(
        PrivacyMaskingSubjectKind::ModelProviderInstance,
        ctx.provider_instance_id,
        &["secret"],
    );
    strict.restore_output = false;
    kernel
        .sync_workspace_policy(
            Uuid::new_v4(),
            ctx.workspace_id,
            None,
            &PrivacyMaskingWorkspacePolicyDto {
                enabled: true,
                allowlist: vec![],
                rules: vec![custom("secret", "SECRET")],
                subject_bindings: vec![strict],
            },
        )
        .await
        .unwrap();
    *repo.fail_run_rewrite.lock().unwrap() = true;
    let err = kernel
        .prepare_invocation(&ctx, &mut ProviderInvocationInput::default())
        .await
        .err()
        .unwrap();
    assert_eq!(err.to_string(), "simulated_run_rewrite_failure");
}

#[test]
fn native_done_flushes_only_the_matching_channel_then_completion_flushes_the_rest() {
    let mut restorer = nested_restorer();
    let mut send = |event| {
        restorer.process_event(ProviderStreamEvent::NativeEvent {
            protocol: "openai_responses".into(),
            event,
        })
    };
    for (item, kind) in [("a", "output_text"), ("b", "refusal")] {
        send(
            json!({"type":format!("response.{kind}.delta"),"item_id":item,"output_index":0,"content_index":0,"delta":"{{OUT"}),
        );
    }
    let done = send(
        json!({"type":"response.output_text.done","item_id":"a","output_index":0,"content_index":0,"text":OUTER}),
    );
    assert_eq!(done.len(), 2);
    let ProviderStreamEvent::NativeEvent { event, .. } = &done[0] else {
        panic!("expected delta")
    };
    assert_eq!(event["delta"], "{{OUT");
    let ProviderStreamEvent::NativeEvent { event, .. } = &done[1] else {
        panic!("expected done")
    };
    assert_eq!(event["text"], format!("<{RAW}>"));
    let completed = send(json!({"type":"response.completed"}));
    assert_eq!(completed.len(), 2);
    let ProviderStreamEvent::NativeEvent { event, .. } = &completed[0] else {
        panic!("expected delta")
    };
    assert_eq!(event["type"], "response.refusal.delta");
    assert_eq!(event["delta"], "{{OUT");
}
