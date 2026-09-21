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
    fixture_with_input(json!({})).await
}

async fn fixture_with_input(
    input_payload: Value,
) -> (
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
            input_payload,
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

fn seed_round_with_response(
    repository: &ApplicationPublicApiTestRepository,
    run: Uuid,
    body: &Value,
    response_id: &str,
) -> CallbackTaskRecord {
    repository.seed_pending_llm_tool_callback_task(run, json!({
        "tool_calls":[{"id":"function","name":"read"},{"id":"custom","name":"exec"}],
        "provider_metadata":{"native_response":{
            "response_id":response_id,
            "configuration_digest":ProviderTransportPayload::openai_responses(body.clone()).unwrap().configuration_digest().unwrap(),
            "user_messages_digest":ProviderTransportPayload::openai_responses(body.clone()).unwrap().user_messages_digest().unwrap()
        }}
    }))
}

fn seed_proven_full_round(
    repository: &ApplicationPublicApiTestRepository,
    run: Uuid,
    include_history: bool,
) -> (CallbackTaskRecord, Value) {
    let mut original = request();
    original["input"] = json!([{"role":"user","content":"Read files"}]);
    let output = vec![
        json!({"type":"function_call","call_id":"function","name":"read","arguments":"{}"}),
        json!({"type":"custom_tool_call","call_id":"custom","name":"exec","input":"read"}),
    ];
    let history = crate::application_public_api::compat::openai::history::completed_history(
        &original, None, &output,
    )
    .unwrap()
    .unwrap();
    let transport = ProviderTransportPayload::openai_responses(original.clone()).unwrap();
    let callback = repository.seed_pending_llm_tool_callback_task(
        run,
        json!({
            "tool_calls":[{"id":"function","name":"read"},{"id":"custom","name":"exec"}],
            "provider_metadata":{"native_response":{
                "response_id":"resp_round",
                "configuration_digest":transport.configuration_digest().unwrap(),
                "user_messages_digest":transport.user_messages_digest().unwrap(),
                "history":if include_history { history } else { Value::Null }
            }}
        }),
    );
    let mut full = original;
    full["input"].as_array_mut().unwrap().extend(output);
    full["input"]
        .as_array_mut()
        .unwrap()
        .extend(request()["input"].as_array().unwrap().iter().cloned());
    (callback, full)
}

#[tokio::test]
async fn native_admission_accepts_proven_full_configuration_refresh_and_completed_replay() {
    for (field, value) in [
        (
            "tools",
            json!([{"type":"function","name":"new_tool","parameters":{"type":"object"}}]),
        ),
        ("instructions", json!("New sampling instructions")),
        ("reasoning", json!({"effort":"high"})),
    ] {
        let (repository, actor, run) =
            fixture_with_input(json!({"sys":{"requested_model_id":"fixture"}})).await;
        let (callback, mut body) = seed_proven_full_round(&repository, run, true);
        body[field] = value;
        for completed in [false, true] {
            if completed {
                repository.complete_callback_task_for_test(callback.id);
            }
            let (admitted, result) =
                correlate_native_responses_callback(&repository, &actor, &body)
                    .await
                    .expect("proven full configuration refresh must reach the callback owner")
                    .unwrap();
            assert_eq!(admitted.id, callback.id, "{field}, completed={completed}");
            assert_eq!(result["tool_results"].as_array().unwrap().len(), 2);
        }
        assert!(repository.callback_resume_attempts().is_empty());
        assert_eq!(repository.flow_run_count(), 1);
    }
}

#[tokio::test]
async fn native_admission_rejects_configuration_refresh_without_frozen_route_and_exact_history() {
    for scenario in [
        "model",
        "delta",
        "missing_history",
        "tampered_history",
        "extra_history",
        "partial_outputs",
        "missing_model",
        "empty_model",
    ] {
        let frozen_input = match scenario {
            "missing_model" => json!({}),
            "empty_model" => json!({"sys":{"requested_model_id":""}}),
            _ => json!({"sys":{"requested_model_id":"fixture"}}),
        };
        let (repository, actor, run) = fixture_with_input(frozen_input).await;
        let (callback, mut body) =
            seed_proven_full_round(&repository, run, scenario != "missing_history");
        body["instructions"] = json!("Changed options require a complete proof");
        match scenario {
            "model" => body["model"] = json!("other-model"),
            "delta" => {
                body["previous_response_id"] = json!("resp_round");
                body["input"] = request()["input"].clone();
            }
            "tampered_history" => body["input"][1]["arguments"] = json!("{\"changed\":true}"),
            "extra_history" => body["input"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"assistant","content":"Unproven extra context"})),
            "partial_outputs" => {
                body["input"].as_array_mut().unwrap().pop();
            }
            _ => {}
        }
        let error = correlate_native_responses_callback(&repository, &actor, &body)
            .await
            .expect_err(scenario);
        let expected = if scenario == "partial_outputs" {
            "native_tool_output_incomplete_round"
        } else {
            "native_tool_output_configuration_mismatch"
        };
        assert_eq!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(&ControlPlaneError::Conflict(expected)),
            "{scenario}"
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
    }
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
async fn native_admission_rejects_a_user_boundary_between_expected_outputs() {
    let (repository, actor, run) = fixture().await;
    let mut body = request();
    seed_round(&repository, run, &body);
    body["input"].as_array_mut().unwrap().insert(
        1,
        json!({"role":"user","content":"do not attach this turn to the old callback"}),
    );
    assert!(
        correlate_native_responses_callback(&repository, &actor, &body)
            .await
            .unwrap()
            .is_none()
    );
    assert!(repository.callback_resume_attempts().is_empty());
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
    let (replayed, replay_payload) =
        correlate_native_responses_callback(&repository, &actor, &body)
            .await
            .expect("completed round must reach the durable replay owner")
            .expect("completed round must still correlate");
    assert_eq!(replayed.id, callback.id);
    assert_eq!(replay_payload["tool_results"].as_array().unwrap().len(), 2);
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

// #2045 AC-003: asynchronous Codex context items after the expected output are
// not callback identity. They must remain in the opaque provider body.
#[tokio::test]
async fn native_admission_accepts_expected_outputs_before_agent_context_items() {
    let (repository, actor, run) = fixture().await;
    let body = request();
    let callback = seed_round(&repository, run, &body);
    let mut request_with_agent_message = body.clone();
    request_with_agent_message["input"]
        .as_array_mut()
        .unwrap()
        .extend([
            json!({"role":"assistant","phase":"commentary","content":"Sub-agent failed"}),
            json!({"type":"reasoning","summary":[]}),
            json!({"type":"future_context_boundary","opaque":true}),
        ]);
    let (actual, result) =
        correlate_native_responses_callback(&repository, &actor, &request_with_agent_message)
            .await
            .expect("context items after outputs are valid Responses history")
            .expect("the pending native callback must correlate");
    assert_eq!(actual.id, callback.id);
    assert_eq!(result["tool_results"].as_array().unwrap().len(), 2);
}

// #2045 AC-004: previous_response_id is the primary causal key. Reused call
// IDs in another round must not make the intended callback ambiguous.
#[tokio::test]
async fn native_admission_uses_previous_response_id_before_call_id_fallback() {
    let (repository, actor, run) = fixture().await;
    let mut body = request();
    let expected = seed_round_with_response(&repository, run, &body, "resp_expected");
    seed_round_with_response(&repository, run, &body, "resp_other");
    body["previous_response_id"] = json!("resp_expected");

    let (actual, _) = correlate_native_responses_callback(&repository, &actor, &body)
        .await
        .expect("response identity should disambiguate reused call IDs")
        .expect("the expected callback must correlate");
    assert_eq!(actual.id, expected.id);
}

// AC-006/008: cancelled waits cannot become Start.
#[tokio::test]
async fn native_admission_rejects_cancelled_waits() {
    let (repository, actor, run) = fixture().await;
    let body = request();
    seed_round(&repository, run, &body);
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

#[derive(Clone)]
struct RecordingNativeCallbackConsumer {
    repository: ApplicationPublicApiTestRepository,
    calls: std::sync::Arc<
        std::sync::Mutex<
            Vec<crate::application_public_api::callback_resume::CompletePublishedCallbackInput>,
        >,
    >,
}

#[async_trait::async_trait]
impl crate::application_public_api::callback_resume::ApplicationPublishedCallbackConsumer
    for RecordingNativeCallbackConsumer
{
    async fn complete_published_callback(
        &self,
        input: crate::application_public_api::callback_resume::CompletePublishedCallbackInput,
    ) -> Result<domain::FlowRunRecord> {
        let callback = self
            .repository
            .get_published_callback_task(input.callback_task_id)
            .await?
            .expect("owned pending callback fixture");
        self.repository.complete_callback_task_for_test(callback.id);
        self.calls.lock().unwrap().push(input);
        Ok(self
            .repository
            .get_published_flow_run(callback.flow_run_id)
            .await?
            .expect("original callback flow"))
    }
}

#[tokio::test]
async fn full_context_extension_configuration_refresh_keeps_pending_callback_owner() {
    use crate::application_public_api::callback_resume::{
        ApplicationPublishedCallbackResumeService, PreparedPublishedCallbackResume,
        PublishedCallbackResumeSource, PublishedCallbackResumeTarget,
        ResumePublishedCallbackCommand,
    };
    let (repository, actor, run) =
        fixture_with_input(json!({"sys":{"requested_model_id":"fixture"}})).await;
    let (callback, mut body) = seed_proven_full_round(&repository, run, true);
    body["input"].as_array_mut().unwrap().extend([
        json!({"type":"reasoning","summary":[]}),
        json!({"role":"assistant","content":"New context"}),
        json!({"role":"developer","content":[{"type":"input_text","text":"Apps became available"}]}),
        json!({"type":"future_context_boundary","opaque":true}),
    ]);
    body["tools"] = json!([{"type":"function","name":"fresh","parameters":{"type":"object"}}]);
    let original = body.clone();
    for mutation in 0..3 {
        let mut invalid = body.clone();
        match mutation {
            0 => invalid["model"] = json!("foreign"),
            1 => invalid["input"][1]["arguments"] = json!("tampered"),
            2 => invalid["input"].as_array_mut().unwrap().push(
                json!({"type":"function_call_output","call_id":"foreign","output":"not this round"}),
            ),
            _ => unreachable!(),
        }
        assert!(
            correlate_native_responses_callback(&repository, &actor, &invalid)
                .await
                .is_err(),
            "pending refresh mutation {mutation}"
        );
    }
    let (admitted, results) = correlate_native_responses_callback(&repository, &actor, &body)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(admitted.id, callback.id);
    assert_eq!(admitted.status, CallbackTaskStatus::Pending);
    assert_eq!(results["tool_results"].as_array().unwrap().len(), 2);
    assert!(repository.callback_resume_attempts().is_empty());

    let consumer = RecordingNativeCallbackConsumer {
        repository: repository.clone(),
        calls: Default::default(),
    };
    let service =
        ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone());
    let mut command = ResumePublishedCallbackCommand {
        transport_connection_scope: None,
        reserved_attempt_id: None,
        native_transport: Some(ProviderTransportPayload::openai_responses(body.clone()).unwrap()),
        bearer_token: String::new(),
        target: PublishedCallbackResumeTarget::CallbackTask {
            callback_task_id: admitted.id,
        },
        source: PublishedCallbackResumeSource::OpenAiResponses,
        response_payload: results.clone(),
        response_mode: Some("streaming".into()),
    };
    let PreparedPublishedCallbackResume::Resume { initial_run } = service
        .prepare_callback_resume_for_actor(actor.clone(), &command)
        .await
        .unwrap()
    else {
        panic!("pending refresh must resume its original callback, never start or recover an invocation");
    };
    assert_eq!(initial_run.id, run);
    command.reserved_attempt_id = Some(
        service
            .reserve_native_callback_for_actor(actor.clone(), &command)
            .await
            .unwrap(),
    );
    for _ in 0..2 {
        let resumed = service
            .resume_callback_for_actor(actor.clone(), command.clone())
            .await
            .unwrap();
        assert_eq!(resumed.run.id, run);
    }
    {
        let calls = consumer.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].callback_task_id, callback.id);
        assert_eq!(calls[0].response_payload, results);
        assert_eq!(
            calls[0].native_transport.as_ref().unwrap().wire_body(),
            &original
        );
    }
    let (completed, _) = correlate_native_responses_callback(&repository, &actor, &body)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed.id, callback.id);
    assert_eq!(completed.status, CallbackTaskStatus::Completed);
    assert_eq!(body, original);
    assert_eq!(repository.callback_resume_attempts().len(), 1);
    assert_eq!(repository.flow_run_count(), 1);

    body["model"] = json!("foreign");
    assert!(
        correlate_native_responses_callback(&repository, &actor, &body)
            .await
            .is_err()
    );
}
