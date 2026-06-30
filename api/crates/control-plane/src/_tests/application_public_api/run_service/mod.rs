use control_plane::application_public_api::{
    api_keys::{ApplicationApiKeyService, CreateApplicationApiKeyCommand},
    mapping::{
        ApplicationApiMappingConfig, ApplicationApiMappingInput, ApplicationApiMappingOutput,
    },
    native::{
        CreateNativeRunCommand, NativeProtocolRequestKind, NativeRunRequest,
        NativeRunValidationError,
    },
    publications::{ApplicationPublicationService, PublishApplicationCommand},
    run_service::{ApplicationPublishedRunControlRepository, ApplicationPublishedRunService},
    ApplicationPublicApiTestHarness,
};
use control_plane::ports::{
    ApplicationEnvironmentVariableInput, ApplicationRepository, FlowRepository,
    ReplaceApplicationEnvironmentVariablesInput,
};
use serde_json::json;
use uuid::Uuid;

fn actor_user_id() -> Uuid {
    Uuid::from_u128(0x11111111111111111111111111111111)
}

fn native_request(response_mode: &str, idempotency_key: Option<&str>) -> NativeRunRequest {
    let execution = idempotency_key
        .map(|key| json!({ "idempotency_key": key }))
        .unwrap_or_else(|| json!({}));
    serde_json::from_value(json!({
        "query": "Summarize the incident",
        "model": "public-model/pass-through",
        "inputs": {
            "priority": "high"
        },
        "conversation": {
            "id": "conversation-1",
            "user": "customer-1"
        },
        "response_mode": response_mode,
        "execution": execution,
        "metadata": {
            "trace_id": "trace-1",
            "request_id": "req-1"
        },
        "compatibility_mode": "native-v1"
    }))
    .unwrap()
}

fn anthropic_request(query: &str) -> NativeRunRequest {
    let mut request: NativeRunRequest = serde_json::from_value(json!({
        "query": query,
        "model": "public-model/pass-through",
        "conversation": {
            "id": "3e7058c2-3120-4222-bb14-c99ec85e1c0f",
            "user": "user_31fb5a_account__session_3e7058c2-3120-4222-bb14-c99ec85e1c0f"
        },
        "response_mode": "streaming",
        "compatibility_mode": "anthropic-messages-v1"
    }))
    .unwrap();
    request.protocol_compatibility_mode = Some("anthropic-messages-v1".to_string());
    request
}

fn anthropic_subagent_request(query: &str) -> NativeRunRequest {
    let mut request = anthropic_request(query);
    request.system = Some(
        "x-anthropic-billing-header: cc_version=2.1.165; cc_entrypoint=cli; cch=007d6; cc_is_subagent=true;\n\nYou are Claude Code."
            .to_string(),
    );
    request
}

fn anthropic_builtin_agent_request(query: &str) -> NativeRunRequest {
    let mut request = anthropic_request(query);
    request.system = Some(
        "x-anthropic-billing-header: cc_version=2.1.141; cc_entrypoint=cli; cch=04e8f;\n\n\
You are Claude Code, Anthropic's official CLI for Claude.\n\n\
You are a file search specialist for Claude Code, Anthropic's official CLI for Claude.\n\n\
Notes:\n\
- Agent threads always have their cwd reset between bash calls, as a result please only use absolute file paths.\n\
- Do NOT Write report/summary/findings/analysis .md files. Return findings directly as your final assistant message — the parent agent reads your text output, not files you create."
            .to_string(),
    );
    request
}

fn anthropic_tool_result_continuation_request(query: &str) -> NativeRunRequest {
    let mut request = anthropic_request(query);
    request.protocol_request_kind =
        Some(NativeProtocolRequestKind::AnthropicToolResultContinuation);
    request
}

fn anthropic_claude_code_control_request(query: &str, control_kind: &str) -> NativeRunRequest {
    let mut request = anthropic_request(query);
    request.inputs = serde_json::from_value(json!({
        "compatibility": {
            "claude_code_control": control_kind
        }
    }))
    .unwrap();
    request
}

fn anthropic_away_summary_request() -> NativeRunRequest {
    anthropic_request(
        "The user stepped away and is coming back. Write exactly 1-3 short sentences. Start by stating the high-level task — what they are building or debugging, not implementation details. Next: the concrete next step. Skip status reports and commit recaps.",
    )
}

fn anthropic_compact_resume_request() -> NativeRunRequest {
    anthropic_request(
        "This session is being continued from a previous conversation that ran out of context. The summary below covers the earlier portion of the conversation.\n\nSummary:\n- user asked where uploads/image-1.png is implemented\n\nContinue the conversation from where it left off without asking the user any further questions. Resume directly — do not acknowledge the summary, do not recap what was happening, do not preface with \"I'll continue\" or similar. Pick up the last task as if the break never happened.",
    )
}

fn native_request_with_model_parameters(
    model: &str,
    model_parameters: serde_json::Value,
) -> NativeRunRequest {
    serde_json::from_value(json!({
        "query": "Summarize the incident",
        "model": model,
        "inputs": {
            "priority": "high"
        },
        "execution": {
            "model_parameters": model_parameters
        }
    }))
    .unwrap()
}

fn published_mapping() -> ApplicationApiMappingConfig {
    ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "node-start.query".into(),
            model_target: None,
            inputs_target: Some("node-start".into()),
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: None,
    }
}

async fn issue_key(harness: &ApplicationPublicApiTestHarness, application_id: Uuid) -> String {
    ApplicationApiKeyService::new(harness.repository())
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id,
            name: "Native runner".into(),
            expires_at: None,
        })
        .await
        .unwrap()
        .token
}

async fn save_start_model_catalog(
    repository: &control_plane::application_public_api::ApplicationPublicApiTestRepository,
    application: &domain::ApplicationRecord,
) {
    let editor_state = repository
        .get_or_create_editor_state(application.workspace_id, application.id, actor_user_id())
        .await
        .unwrap();
    let mut document = editor_state.draft.document;
    let start_node = document["graph"]["nodes"]
        .as_array_mut()
        .expect("nodes array")
        .iter_mut()
        .find(|node| node["type"] == "start")
        .expect("default document has a start node");
    start_node["config"]["model_list"] = json!([
        {
            "id": "gpt-5.4",
            "name": "GPT-5.4",
            "max_output_tokens": 32000,
            "capabilities": {
                "reasoning": true
            },
            "reasoning": {
                "default_effort": "medium",
                "supported_efforts": ["low", "medium", "high"]
            }
        },
        {
            "id": "plain-model",
            "name": "Plain model"
        }
    ]);
    FlowRepository::save_draft(
        repository,
        application.workspace_id,
        application.id,
        actor_user_id(),
        document,
        domain::FlowChangeKind::Logical,
        "Configure published model catalog",
    )
    .await
    .unwrap();
}

mod anthropic_parent_callbacks;
mod native_results;
mod native_start;
mod publication_guards;
