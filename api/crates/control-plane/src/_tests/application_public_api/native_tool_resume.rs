use super::*;
use crate::application_public_api::{
    ApplicationPublicApiTestHarness, ApplicationPublicApiTestRepository,
};
use control_plane_contracts::{
    application_public_runtime::ApplicationPublishedFlowRunRepository, ports::CreateFlowRunInput,
};
use time::OffsetDateTime;
use uuid::Uuid;

async fn fixture() -> (
    ApplicationPublicApiTestRepository,
    ApplicationApiKeyActor,
    Uuid,
) {
    let harness = ApplicationPublicApiTestHarness::new();
    let owner = Uuid::now_v7();
    let app = harness.seed_application(owner, "Native admission");
    let repository = harness.repository();
    let actor = ApplicationApiKeyActor {
        api_key_id: Uuid::now_v7(),
        application_id: app.id,
        creator_user_id: owner,
        tenant_id: Uuid::now_v7(),
        workspace_id: app.workspace_id,
        actor: domain::ActorContext::scoped_in_scope(
            owner,
            Uuid::nil(),
            app.workspace_id,
            "application_api_key",
            Vec::<String>::new(),
        ),
    };
    let run = repository
        .create_published_flow_run(&CreateFlowRunInput {
            application_run_log_context: None,
            actor_user_id: owner,
            application_id: app.id,
            flow_id: Uuid::now_v7(),
            flow_draft_id: Uuid::now_v7(),
            compiled_plan_id: Uuid::now_v7(),
            debug_session_id: String::new(),
            flow_schema_version: "1".into(),
            document_hash: "fixture".into(),
            run_mode: domain::FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "Native admission".into(),
            status: FlowRunStatus::Running,
            input_payload: json!({}),
            started_at: OffsetDateTime::now_utc(),
            api_key_id: Some(actor.api_key_id),
            publication_version_id: None,
            assistant_conversation_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: Some("openai-responses".into()),
            idempotency_key: None,
        })
        .await
        .unwrap()
        .flow_run;
    (repository, actor, run.id)
}

fn request() -> Value {
    json!({
        "model":"fixture", "instructions":"Read sequentially", "tools":[],
        "input":[
            {"type":"function_call_output","call_id":"function","output":"first result"},
            {"type":"custom_tool_call_output","call_id":"custom","output":[{"type":"input_text","text":"second result"},{"type":"input_image","image_url":"data:image/png;base64,AA=="}]}
        ]
    })
}

fn seed_round(
    repository: &ApplicationPublicApiTestRepository,
    run: Uuid,
    body: &Value,
) -> CallbackTaskRecord {
    repository.seed_pending_llm_tool_callback_task(run, json!({
        "tool_calls":[{"id":"function","name":"read"},{"id":"custom","name":"exec"}],
        "provider_metadata":{"native_response":{
            "response_id":"resp_round",
            "configuration_digest":ProviderTransportPayload::openai_responses(body.clone()).unwrap().configuration_digest().unwrap(),
            "user_messages_digest":ProviderTransportPayload::openai_responses(body.clone()).unwrap().user_messages_digest().unwrap()
        }}
    }))
}

// #2036: HTTP full histories and WS deltas resume the same round without
// replaying older outputs; multimodal/custom output stays a structured value.
#[tokio::test]
async fn native_admission_correlates_current_suffix_and_preserves_output_values() {
    let (repository, actor, run) = fixture().await;
    let body = request();
    let callback = seed_round(&repository, run, &body);
    let expected = json!({"tool_results":[
        {"tool_call_id":"function","content":body["input"][0]["output"]},
        {"tool_call_id":"custom","content":body["input"][1]["output"]}
    ]});
    let mut full = body.clone();
    let mut history = vec![
        json!({"role":"user","content":"Read files"}),
        json!({"type":"function_call_output","call_id":"old","output":"historical"}),
        json!({"type":"function_call","call_id":"function","name":"read","arguments":"{}"}),
    ];
    history.extend(body["input"].as_array().unwrap().iter().cloned());
    full["input"] = json!(history);
    let mut delta = body.clone();
    delta["previous_response_id"] = json!("resp_round");
    delta["stream"] = json!(true);
    delta["metadata"] = json!({"request":"new"});
    for candidate in [full, delta] {
        let (actual, result) = correlate_native_responses_callback(&repository, &actor, &candidate)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(actual.id, callback.id);
        assert_eq!(actual.request_payload, callback.request_payload);
        assert_eq!(result, expected);
    }
    assert_eq!(repository.flow_run_count(), 1);
    assert!(repository.callback_resume_attempts().is_empty());
}

#[tokio::test]
async fn native_admission_leaves_history_before_new_user_turn_alone() {
    let (repository, actor, run) = fixture().await;
    let mut body = request();
    seed_round(&repository, run, &body);
    body["input"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"user","content":"Next task"}));
    assert!(
        correlate_native_responses_callback(&repository, &actor, &body)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn native_admission_rejects_partial_unknown_duplicate_and_changed_rounds() {
    let (repository, actor, run) = fixture().await;
    let body = request();
    let callback = seed_round(&repository, run, &body);
    let mut partial = body.clone();
    partial["input"].as_array_mut().unwrap().pop();
    let mut unknown = body.clone();
    unknown["input"][1]["call_id"] = json!("unknown");
    let mut duplicate = body.clone();
    duplicate["input"][1]["call_id"] = json!("function");
    let mut previous = body.clone();
    previous["previous_response_id"] = json!("resp_other");
    let mut changed = body.clone();
    changed["model"] = json!("other-model");
    for (candidate, code) in [
        (partial, "native_tool_output_incomplete_round"),
        (unknown, "native_tool_output_incomplete_round"),
        (duplicate, "native_tool_output_duplicate"),
        (previous, "native_tool_output_response_mismatch"),
        (changed, "native_tool_output_configuration_mismatch"),
    ] {
        let error = correlate_native_responses_callback(&repository, &actor, &candidate)
            .await
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(&ControlPlaneError::Conflict(code))
        );
    }
    assert_eq!(
        repository
            .get_published_callback_task(callback.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        CallbackTaskStatus::Pending
    );
    repository.complete_callback_task_for_test(callback.id);
    let error = correlate_native_responses_callback(&repository, &actor, &body)
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(&ControlPlaneError::Conflict(
            "native_tool_output_round_not_pending"
        ))
    );
    assert!(repository.callback_resume_attempts().is_empty());
}

#[tokio::test]
async fn native_admission_requires_one_owned_callback_round() {
    let (repository, actor, run) = fixture().await;
    let body = request();
    seed_round(&repository, run, &body);
    let mut foreign_key = actor.clone();
    foreign_key.api_key_id = Uuid::now_v7();
    let mut foreign_owner = actor.clone();
    foreign_owner.creator_user_id = Uuid::now_v7();
    let mut foreign_workspace = actor.clone();
    foreign_workspace.workspace_id = Uuid::now_v7();
    let mut foreign_application = actor.clone();
    foreign_application.application_id = Uuid::now_v7();
    for foreign in [
        foreign_key,
        foreign_owner,
        foreign_workspace,
        foreign_application,
    ] {
        let error = correlate_native_responses_callback(&repository, &foreign, &body)
            .await
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(&ControlPlaneError::Conflict("native_tool_output_unknown"))
        );
    }
    seed_round(&repository, run, &body);
    let error = correlate_native_responses_callback(&repository, &actor, &body)
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(&ControlPlaneError::Conflict(
            "native_tool_output_ambiguous_round"
        ))
    );
    assert_eq!(repository.flow_run_count(), 1);
}

#[test]
fn configuration_digest_ignores_delivery_history_and_json_key_order_only() {
    let baseline = ProviderTransportPayload::openai_responses(
        json!({"model":"m","reasoning":{"effort":"high","summary":"auto"},"input":[]}),
    )
    .unwrap()
    .configuration_digest()
    .unwrap();
    let reordered = ProviderTransportPayload::openai_responses(json!({"reasoning":{"summary":"auto","effort":"high"},"model":"m","input":["new"],"previous_response_id":"resp_1","stream":true,"stream_options":{},"client_metadata":{},"metadata":{}})).unwrap().configuration_digest().unwrap();
    assert_eq!(baseline, reordered);
    let changed = ProviderTransportPayload::openai_responses(
        json!({"model":"m","reasoning":{"effort":"low","summary":"auto"},"input":[]}),
    )
    .unwrap()
    .configuration_digest()
    .unwrap();
    assert_ne!(baseline, changed);
}

// #2036: a new user turn cannot reuse an old pending tool suffix even when its
// call IDs, previous response and generation configuration are unchanged.
#[tokio::test]
async fn native_admission_vetoes_old_outputs_after_a_new_user_message() {
    let (repository, actor, run) = fixture().await;
    let mut original = request();
    original["input"].as_array_mut().unwrap().insert(
        0,
        json!({"role":"user","content":[{"type":"input_text","text":"Read the first file"}]}),
    );
    let callback = seed_round(&repository, run, &original);
    assert!(callback
        .request_payload
        .pointer("/provider_metadata/native_response/user_messages_digest")
        .and_then(Value::as_str)
        .is_some());
    let (admitted, _) = correlate_native_responses_callback(&repository, &actor, &original)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(admitted.id, callback.id);

    let mut next_turn = original.clone();
    next_turn["input"].as_array_mut().unwrap().insert(
        1,
        json!({"role":"user","content":[{"type":"input_text","text":"Start a different task"}]}),
    );
    let error = correlate_native_responses_callback(&repository, &actor, &next_turn)
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(&ControlPlaneError::Conflict(
            "native_tool_output_user_turn_mismatch"
        ))
    );
    assert_eq!(
        repository
            .get_published_callback_task(callback.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        CallbackTaskStatus::Pending
    );
    assert!(repository.callback_resume_attempts().is_empty());
    assert_eq!(repository.flow_run_count(), 1);
}

// AC-006/008: malformed continuation and cancelled waits cannot become Start.
#[tokio::test]
async fn native_admission_rejects_non_tail_outputs_and_cancelled_waits() {
    let (repository, actor, run) = fixture().await;
    let body = request();
    seed_round(&repository, run, &body);
    let mut malformed = body.clone();
    malformed["input"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"assistant","content":"stale answer"}));
    let error = correlate_native_responses_callback(&repository, &actor, &malformed)
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(&ControlPlaneError::Conflict("native_tool_output_not_tail"))
    );
    repository
        .cancel_published_pending_callback_tasks_for_run(run, OffsetDateTime::now_utc())
        .await
        .unwrap();
    let error = correlate_native_responses_callback(&repository, &actor, &body)
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(&ControlPlaneError::Conflict(
            "native_tool_output_round_not_pending"
        ))
    );
    assert_eq!(repository.flow_run_count(), 1);
    assert!(repository.callback_resume_attempts().is_empty());
}
