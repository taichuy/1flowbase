use plugin_framework::provider_contract::ProviderInvocationInput;
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::orchestration_runtime::scheduler_admission::{
    derive_provider_pool_key, derive_scheduler_admission_metadata,
};

fn flow_run(
    compatibility_mode: Option<&str>,
    external_user: Option<&str>,
    external_conversation_id: Option<&str>,
    debug_session_id: &str,
) -> domain::FlowRunRecord {
    let now = OffsetDateTime::now_utc();
    domain::FlowRunRecord {
        id: Uuid::now_v7(),
        application_id: Uuid::parse_str("00000000-0000-7000-8000-000000000001").unwrap(),
        flow_id: Uuid::parse_str("00000000-0000-7000-8000-000000000002").unwrap(),
        draft_id: Uuid::parse_str("00000000-0000-7000-8000-000000000003").unwrap(),
        compiled_plan_id: Some(Uuid::parse_str("00000000-0000-7000-8000-000000000004").unwrap()),
        debug_session_id: debug_session_id.to_string(),
        flow_schema_version: "1.0.0".to_string(),
        document_hash: "hash".to_string(),
        run_mode: domain::FlowRunMode::PublishedApiRun,
        target_node_id: None,
        title: "Fixture run".to_string(),
        status: domain::FlowRunStatus::Queued,
        input_payload: json!({}),
        output_payload: json!({}),
        error_payload: None,
        created_by: Uuid::parse_str("00000000-0000-7000-8000-000000000005").unwrap(),
        authorized_account: None,
        api_key_id: Some(Uuid::parse_str("00000000-0000-7000-8000-000000000006").unwrap()),
        publication_version_id: Some(
            Uuid::parse_str("00000000-0000-7000-8000-000000000007").unwrap(),
        ),
        external_user: external_user.map(ToOwned::to_owned),
        external_conversation_id: external_conversation_id.map(ToOwned::to_owned),
        external_trace_id: None,
        compatibility_mode: compatibility_mode.map(ToOwned::to_owned),
        idempotency_key: None,
        started_at: now,
        finished_at: None,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn published_api_session_key_ignores_legacy_compatibility_mode() {
    let without_mode = flow_run(None, Some("claude-root-session"), Some("turn-a"), "");
    let with_legacy_mode = flow_run(
        Some("anthropic-messages-v1"),
        Some("claude-root-session"),
        Some("turn-a"),
        "",
    );

    let without_mode = derive_scheduler_admission_metadata(&without_mode);
    let with_legacy_mode = derive_scheduler_admission_metadata(&with_legacy_mode);

    assert!(!without_mode.stateless);
    assert_eq!(
        without_mode.client_session_key,
        with_legacy_mode.client_session_key
    );
    assert_eq!(
        without_mode.run_admission_key,
        with_legacy_mode.run_admission_key
    );
    assert!(!without_mode
        .client_session_key
        .as_deref()
        .unwrap()
        .contains("claude-root-session"));
}

#[test]
fn openai_responses_external_conversation_changes_do_not_change_root_session_key() {
    let first = flow_run(
        None,
        Some("codex-root-session"),
        Some("response-thread-a"),
        "",
    );
    let second = flow_run(
        Some("forged-protocol-mode"),
        Some("codex-root-session"),
        Some("response-thread-b"),
        "",
    );

    let first = derive_scheduler_admission_metadata(&first);
    let second = derive_scheduler_admission_metadata(&second);

    assert_eq!(first.client_session_key, second.client_session_key);
    assert_eq!(first.run_admission_key, second.run_admission_key);
}

#[test]
fn openai_chat_without_session_signal_is_stateless() {
    let run = flow_run(Some("openai-chat-completions-v1"), None, None, "");

    let metadata = derive_scheduler_admission_metadata(&run);

    assert!(metadata.stateless);
    assert!(metadata.client_session_key.is_none());
    assert!(metadata.run_admission_key.contains(&run.id.to_string()));
}

#[test]
fn native_debug_run_uses_debug_session_without_raw_session_text() {
    let mut run = flow_run(None, None, None, "local-debug-session");
    run.run_mode = domain::FlowRunMode::DebugFlowRun;
    run.api_key_id = None;

    let metadata = derive_scheduler_admission_metadata(&run);

    assert!(!metadata.stateless);
    assert!(metadata
        .client_session_key
        .as_deref()
        .is_some_and(|key| key.contains("run_mode=debug_flow_run")));
    assert!(!metadata
        .client_session_key
        .as_deref()
        .unwrap()
        .contains("local-debug-session"));
}

#[test]
fn provider_pool_key_uses_provider_protocol_and_model() {
    let mut input = ProviderInvocationInput {
        provider_instance_id: "provider-1".to_string(),
        provider_code: "openai".to_string(),
        protocol: "openai_responses".to_string(),
        model: "1flowbase".to_string(),
        ..ProviderInvocationInput::default()
    };

    let first = derive_provider_pool_key(&input);
    input.model = "other-model".to_string();
    let second = derive_provider_pool_key(&input);

    assert!(first.contains("provider_code=openai"));
    assert!(first.contains("protocol=openai_responses"));
    assert_ne!(first, second);
}
