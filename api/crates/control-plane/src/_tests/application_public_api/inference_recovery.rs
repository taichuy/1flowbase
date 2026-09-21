use anyhow::Result;
use async_trait::async_trait;
use extension_contracts::provider_contract::{
    ProtocolContextEnvelope, CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::application_public_api::{
    api_keys::{ApplicationApiKeyService, CreateApplicationApiKeyCommand},
    callback_resume::{
        ApplicationPublishedCallbackAttemptRepository, ApplicationPublishedCallbackConsumer,
        ApplicationPublishedCallbackResumeService, CompletePublishedCallbackInput,
        PreparedPublishedCallbackResume, PublishedCallbackResumeSource,
        PublishedCallbackResumeTarget, ResumePublishedCallbackCommand,
    },
    compat::openai::history::completed_history,
    mapping::{
        ApplicationApiMappingConfig, ApplicationApiMappingInput, ApplicationApiMappingOutput,
    },
    native::{
        ApplicationNativeRunService, CreateNativeRunCommand, NativeExecutionModelParameters,
        NativeRunRequest,
    },
    protocol_translation::TranslationProtocol,
    publications::{ApplicationPublicationService, PublishApplicationCommand},
    run_service::ApplicationPublishedRunControlRepository,
    ApplicationPublicApiTestHarness, ApplicationPublicApiTestRepository,
};
use crate::ports::{
    FlowRepository, ProviderProtocolContextLocator, ProviderProtocolContextValue,
    ProviderTransportPayload,
};

#[derive(Clone)]
struct SettlingConsumer {
    repository: ApplicationPublicApiTestRepository,
    failure: Option<Value>,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl ApplicationPublishedCallbackConsumer for SettlingConsumer {
    async fn complete_published_callback(
        &self,
        input: CompletePublishedCallbackInput,
    ) -> Result<domain::FlowRunRecord> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let callback = self
            .repository
            .get_published_callback_task(input.callback_task_id)
            .await?
            .unwrap();
        let result = if let Some(error) = &self.failure {
            self.repository
                .fail_waiting_callback_published_run(
                    callback.flow_run_id,
                    error.clone(),
                    OffsetDateTime::now_utc(),
                )
                .await?
        } else {
            self.repository
                .complete_waiting_callback_published_internal_run(
                    callback.flow_run_id,
                    json!({"answer":"done"}),
                    OffsetDateTime::now_utc(),
                )
                .await?
        };
        self.repository.complete_callback_task_for_test(callback.id);
        Ok(result.expect("waiting callback fixture"))
    }
}

type Service =
    ApplicationPublishedCallbackResumeService<ApplicationPublicApiTestRepository, SettlingConsumer>;
struct Fixture {
    service: Service,
    repository: ApplicationPublicApiTestRepository,
    consumer: SettlingConsumer,
    command: ResumePublishedCallbackCommand,
    flow_run_id: Uuid,
    callback_task_id: Uuid,
}

fn inference_binding() -> Value {
    json!({"provider_configuration_digest":"sha256:fixture", "provider_instance_id":Uuid::from_u128(99).to_string(),
        "node_id":"node-published-llm", "provider_code":"fixture", "protocol":"responses", "model":"fixture",
        "installation_id":Uuid::from_u128(100), "plugin_version":"fixture", "artifact_checksum":"fixture"})
}

fn transport_failure() -> Value {
    let deadline = (OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64 + 60_000;
    json!({"error_code":"provider_transport_unavailable", "failed_after_first_token":false,
    "native_inference_binding":inference_binding(),
    "provider_instance_id":Uuid::from_u128(99).to_string(),
    "ai_native_recovery":{
        "decision":"non_reproducible", "provider_final_commit":"lifecycle_only",
        "total_attempt_budget":3, "total_attempts_charged":1,
        "provider_directive":{"policy":{"type":"native_opaque","budget":{"max_inner_attempts":1,"absolute_deadline_unix_ms":deadline}},"transport_epoch":1,"initial_commit_level":"lifecycle_only"},
        "provider_inner_receipt":{"attempt":0,"transport":"ai_native_web_socket","transport_epoch":1,"socket_incarnation":1,"commit_level":"lifecycle_only","disposition":"logical_invocation_retry","reason":"transport_disconnected"}
    }})
}

async fn fixture(failure: Option<Value>) -> Fixture {
    fixture_with_configuration(failure, None).await
}

async fn fixture_with_configuration(failure: Option<Value>, tools: Option<Value>) -> Fixture {
    fixture_with_protocol_context(failure, tools, None).await
}

async fn fixture_with_protocol_context(
    mut failure: Option<Value>,
    tools: Option<Value>,
    protocol_context: Option<ProtocolContextEnvelope>,
) -> Fixture {
    let actor = Uuid::from_u128(0x11111111111111111111111111111111);
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor, "Native inference recovery");
    let repository = harness.repository();
    let editor_state = repository
        .get_or_create_editor_state(application.workspace_id, application.id, actor)
        .await
        .unwrap();
    let mut document = editor_state.draft.document;
    let start = document["graph"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["type"] == "start")
        .unwrap();
    start["config"]["model_list"] = json!(["fixture"]);
    FlowRepository::save_draft(
        &repository,
        application.workspace_id,
        application.id,
        actor,
        document,
        domain::FlowChangeKind::Logical,
        "Configure native recovery public model fixture",
    )
    .await
    .unwrap();
    let token = ApplicationApiKeyService::new(repository.clone())
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor,
            application_id: application.id,
            name: "recovery".into(),
            expires_at: None,
        })
        .await
        .unwrap()
        .token;
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor,
            application_id: application.id,
            api_enabled: true,
            mapping: ApplicationApiMappingConfig {
                input: ApplicationApiMappingInput {
                    query_target: "node-start.query".into(),
                    model_target: None,
                    inputs_target: None,
                    history_target: None,
                    attachments_target: None,
                },
                output: ApplicationApiMappingOutput::default(),
                extension: None,
            },
        })
        .await
        .unwrap();
    repository.configure_runnable_published_generate_route(application.id);
    let mut request: NativeRunRequest =
        serde_json::from_value(json!({"query":"work", "model":"fixture"})).unwrap();
    request.client_protocol_envelope = protocol_context;
    let run = ApplicationNativeRunService::new(repository.clone())
        .create_native_run(CreateNativeRunCommand {
            protocol: TranslationProtocol::Native,
            bearer_token: token.clone(),
            request,
        })
        .await
        .unwrap();
    let original = json!({"model":"fixture", "input":[{"role":"user","content":"work"}]});
    let output = vec![
        json!({"type":"reasoning","id":"rs_1","summary":[],"encrypted_content":"opaque-reasoning"}),
        json!({"type":"compaction","encrypted_content":"opaque-compaction"}),
        json!({"type":"message","id":"msg_1","role":"assistant","content":[{"type":"output_text","text":"working"}],"phase":"commentary"}),
        json!({"type":"function_call","id":"fc_1","call_id":"call_1","name":"exec","arguments":"{}"}),
    ];
    let history = completed_history(&original, None, &output)
        .unwrap()
        .unwrap();
    let flow = repository
        .get_published_flow_run(run.id)
        .await
        .unwrap()
        .unwrap();
    let sealed = NativeExecutionModelParameters::seal_published_reasoning_default(
        &flow.input_payload,
        ProviderTransportPayload::openai_responses(original.clone()).unwrap(),
    )
    .unwrap();
    let callback = repository.seed_pending_llm_tool_callback_task(run.id, json!({
        "tool_calls":[{"id":"call_1","name":"exec","arguments":{}}],
        "provider_metadata":{"native_response":{"binding":inference_binding(),"configuration_digest":sealed.configuration_digest().unwrap(),"history":history}}
    }));
    let mut body = original;
    body["input"].as_array_mut().unwrap().extend(output);
    body["input"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type":"function_call_output","call_id":"call_1","output":"done"}));
    if let Some(tools) = tools {
        body["tools"] = tools;
        if let Some(error) = failure.as_mut() {
            let failed_request = NativeExecutionModelParameters::seal_published_reasoning_default(
                &flow.input_payload,
                ProviderTransportPayload::openai_responses(body.clone()).unwrap(),
            )
            .unwrap();
            error["native_inference_configuration_digest"] =
                json!(failed_request.configuration_digest().unwrap());
        }
    }
    let command = ResumePublishedCallbackCommand {
        transport_connection_scope: None,
        reserved_attempt_id: None,
        native_transport: Some(ProviderTransportPayload::openai_responses(body).unwrap()),
        bearer_token: token,
        target: PublishedCallbackResumeTarget::FlowRun {
            flow_run_id: run.id,
            callback_task_id: callback.id,
        },
        source: PublishedCallbackResumeSource::OpenAiResponses,
        response_payload: json!({"tool_results":[{"tool_call_id":"call_1","content":"done"}]}),
        response_mode: Some("blocking".into()),
    };
    let consumer = SettlingConsumer {
        repository: repository.clone(),
        failure,
        calls: Arc::new(AtomicUsize::new(0)),
    };
    let service = Service::new(repository.clone(), consumer.clone());
    service
        .resume_callback(command.clone())
        .await
        .expect("first receipt consumed once");
    Fixture {
        service,
        repository,
        consumer,
        command,
        flow_run_id: run.id,
        callback_task_id: callback.id,
    }
}

impl Fixture {
    async fn prepare(
        &self,
        command: &ResumePublishedCallbackCommand,
    ) -> Result<PreparedPublishedCallbackResume> {
        let actor = ApplicationApiKeyService::new(self.repository.clone())
            .authenticate_bearer_token(&command.bearer_token)
            .await?;
        self.service
            .prepare_callback_resume_for_actor(actor, command)
            .await
    }
    fn assert_receipt_unchanged(&self) {
        assert_eq!(self.consumer.calls.load(Ordering::SeqCst), 1);
        assert_eq!(self.repository.callback_resume_attempts().len(), 1);
    }
}

#[tokio::test]
async fn native_full_context_precommit_transport_admits_inference_only() {
    let f = fixture(Some(transport_failure())).await;
    let public_callback = f
        .repository
        .get_published_callback_task(f.callback_task_id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        public_callback
            .request_payload
            .get("provider_metadata")
            .is_none(),
        "public projection stays private-state-free"
    );
    let PreparedPublishedCallbackResume::RecoverInference { grant } =
        f.prepare(&f.command).await.unwrap()
    else {
        panic!("must admit inference reconstruction")
    };
    assert_eq!(grant.callback_task_id, f.callback_task_id);
    assert_eq!(grant.failed_flow_run_id, f.flow_run_id);
    assert_eq!(grant.remaining_attempts, 2);
    assert_eq!(grant.node_id, "node-published-llm");
    assert_eq!(grant.binding, inference_binding());
    assert_eq!(grant.provider_instance_id, Uuid::from_u128(99).to_string());
    assert_eq!(
        f.repository
            .get_published_flow_run(f.flow_run_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        domain::FlowRunStatus::Failed
    );
    assert_eq!(
        f.repository
            .get_published_callback_task(f.callback_task_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        domain::CallbackTaskStatus::Completed
    );
    f.assert_receipt_unchanged();
}

#[tokio::test]
async fn native_recovery_rejects_missing_changed_history_and_configuration() {
    let f = fixture(Some(transport_failure())).await;
    assert!(matches!(
        f.prepare(&f.command).await.unwrap(),
        PreparedPublishedCallbackResume::RecoverInference { .. }
    ));
    for mutation in 0..8 {
        let mut command = f.command.clone();
        if mutation == 0 {
            command.native_transport = None;
        } else {
            let mut body = command.native_transport.take().unwrap().into_wire_body();
            match mutation {
                1 => {
                    body["input"].as_array_mut().unwrap().remove(1);
                }
                2 => body["input"][1]["encrypted_content"] = json!("altered"),
                3 => body["input"][2]["encrypted_content"] = json!("altered"),
                4 => body["input"][3]["content"][0]["text"] = json!("altered"),
                5 => body["input"][4]["arguments"] = json!("altered"),
                6 => body["model"] = json!("different"),
                7 => body["previous_response_id"] = json!("resp_stale"),
                _ => unreachable!(),
            }
            command.native_transport =
                Some(ProviderTransportPayload::openai_responses(body).unwrap());
        }
        let expected = match mutation {
            0 | 7 => "native_recovery_full_context_required",
            1 => "native_recovery_history_item_count_mismatch",
            6 => "native_recovery_configuration_mismatch",
            _ => "native_recovery_history_mismatch",
        };
        let error = f.prepare(&command).await.unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("conflict: {expected}"),
            "mutation {mutation}"
        );
    }
    f.assert_receipt_unchanged();
}

#[tokio::test]
async fn private_recovery_evidence_stays_in_the_api_actor_scope() {
    let f = fixture(Some(transport_failure())).await;
    let actor = ApplicationApiKeyService::new(f.repository.clone())
        .authenticate_bearer_token(&f.command.bearer_token)
        .await
        .unwrap();
    assert!(matches!(
        f.prepare(&f.command).await.unwrap(),
        PreparedPublishedCallbackResume::RecoverInference { .. }
    ));
    for field in 0..4 {
        let mut other = actor.clone();
        let foreign = Uuid::from_u128(0x99999999999999999999999999999999);
        match field {
            0 => other.workspace_id = foreign,
            1 => other.application_id = foreign,
            2 => other.api_key_id = foreign,
            3 => other.creator_user_id = foreign,
            _ => unreachable!(),
        }
        let error = f
            .service
            .prepare_callback_resume_for_actor(other, &f.command)
            .await
            .unwrap_err();
        if field == 0 {
            assert_eq!(
                error.to_string(),
                "conflict: native_recovery_history_missing"
            );
        } else {
            assert!(matches!(
                error.downcast_ref::<crate::errors::ControlPlaneError>(),
                Some(crate::errors::ControlPlaneError::PermissionDenied(
                    "application_public_callback_resume"
                ))
            ));
        }
    }
    assert_eq!(f.repository.flow_run_count(), 1);
    f.assert_receipt_unchanged();
}

#[tokio::test]
async fn native_recovery_rejects_semantic_untrusted_expired_and_exhausted_failures() {
    for mutation in 0..7 {
        let mut failure = transport_failure();
        match mutation {
            0 => failure["failed_after_first_token"] = json!(true),
            1 => {
                failure["ai_native_recovery"]["provider_final_commit"] =
                    json!("semantic_committed");
            }
            2 => {
                failure["ai_native_recovery"]["provider_inner_receipt"]["commit_level"] =
                    json!("semantic_committed");
            }
            3 => failure["ai_native_recovery"]["provider_inner_receipt"] = Value::Null,
            4 => {
                failure["ai_native_recovery"]["provider_directive"]["policy"]["budget"]
                    ["absolute_deadline_unix_ms"] = json!(1)
            }
            5 => failure["ai_native_recovery"]["total_attempts_charged"] = json!(3),
            6 => {
                failure["ai_native_recovery"]["provider_inner_receipt"]["reason"] =
                    json!("protocol_error")
            }
            _ => unreachable!(),
        }
        let f = fixture(Some(failure)).await;
        assert!(
            f.prepare(&f.command).await.is_err(),
            "failure mutation {mutation} must reject"
        );
        f.assert_receipt_unchanged();
    }
}

#[tokio::test]
async fn successful_callback_duplicate_keeps_original_run_and_consumes_once() {
    let f = fixture(None).await;
    let PreparedPublishedCallbackResume::Resume { initial_run } =
        f.prepare(&f.command).await.unwrap()
    else {
        panic!("success must replay")
    };
    assert_eq!(initial_run.id, f.flow_run_id);
    let replay = f.service.resume_callback(f.command.clone()).await.unwrap();
    assert_eq!(replay.run.id, f.flow_run_id);
    assert_eq!(
        replay.run.status,
        crate::application_public_api::native::NativeRunStatus::Succeeded
    );
    f.assert_receipt_unchanged();
}

#[tokio::test]
async fn recovery_successor_duplicates_project_same_linked_run() {
    assert_recovery_successor_context(None).await;
}

#[tokio::test]
async fn recovery_successor_rebinds_changed_protocol_context_locator() {
    assert_recovery_successor_context(Some(recovery_protocol_context("current-http"))).await;
}

#[tokio::test]
async fn recovery_successor_preserves_matching_protocol_context_locator() {
    assert_recovery_successor_context(Some(recovery_protocol_context("original-ws"))).await;
}

fn recovery_protocol_context(session: &str) -> ProtocolContextEnvelope {
    ProtocolContextEnvelope {
        source_protocol: "openai_responses".into(),
        headers: std::collections::BTreeMap::from([("session-id".into(), vec![session.into()])]),
        ..Default::default()
    }
}

async fn assert_recovery_successor_context(current_context: Option<ProtocolContextEnvelope>) {
    let original_context = recovery_protocol_context("original-ws");
    let f = fixture_with_protocol_context(
        Some(transport_failure()),
        None,
        Some(original_context.clone()),
    )
    .await;
    let PreparedPublishedCallbackResume::RecoverInference { grant } =
        f.prepare(&f.command).await.unwrap()
    else {
        panic!("recovery grant")
    };
    let mut request: NativeRunRequest =
        serde_json::from_value(json!({"query":"reconstructed"})).unwrap();
    request.client_protocol_envelope = current_context.clone();
    let frozen_input = grant.frozen_input_payload.clone();
    let expected_budget = grant.remaining_attempts;
    let expected_deadline = grant.absolute_deadline_unix_ms;
    request.metadata.set_inference_recovery(*grant);
    let actor = ApplicationApiKeyService::new(f.repository.clone())
        .authenticate_bearer_token(&f.command.bearer_token)
        .await
        .unwrap();
    let original_count = f.repository.flow_run_count();
    let barrier = Arc::new(tokio::sync::Barrier::new(16));
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let barrier = barrier.clone();
        let repository = f.repository.clone();
        let actor = actor.clone();
        let request = request.clone();
        tasks.spawn(async move {
            barrier.wait().await;
            ApplicationNativeRunService::new(repository)
                .create_native_run_for_actor(actor, request, TranslationProtocol::Native)
                .await
        });
    }
    let mut winners = Vec::new();
    let mut losers = 0;
    while let Some(result) = tasks.join_next().await {
        match result.expect("create task must not panic") {
            Ok(run) => winners.push(run),
            Err(crate::application_public_api::native::NativeRunValidationError::IdempotencyConflict) => losers += 1,
            Err(error) => panic!("unexpected admission failure: {error:?}"),
        }
    }
    assert_eq!(
        winners.len(),
        1,
        "one inference execution owner among 16 submissions"
    );
    assert_eq!(losers, 15);
    assert_eq!(f.repository.flow_run_count(), original_count + 1);
    let successor = winners.pop().unwrap();
    assert_ne!(successor.id, f.flow_run_id);
    let persisted = f
        .repository
        .get_published_flow_run(successor.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        persisted.idempotency_key,
        Some(format!("native-inference-recovery:{}", f.callback_task_id))
    );
    assert_eq!(
        persisted.input_payload["sys"]["native_inference_recovery"]["remaining_attempts"],
        json!(expected_budget)
    );
    assert_eq!(
        persisted.input_payload["sys"]["native_inference_recovery"]["absolute_deadline_unix_ms"],
        json!(expected_deadline)
    );
    assert_eq!(
        persisted.input_payload["sys"]["native_inference_recovery"]["failed_flow_run_id"],
        json!(f.flow_run_id)
    );
    assert_eq!(
        persisted.input_payload["node-start"], frozen_input["node-start"],
        "recovery must not remap the replacement query into original workflow input"
    );
    assert_eq!(persisted.input_payload["env"], frozen_input["env"]);
    let old_locator_value = &frozen_input[CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY];
    let old_locator = ProviderProtocolContextLocator::parse(old_locator_value)
        .unwrap()
        .expect("the predecessor has a sealed protocol context locator");
    match current_context {
        Some(envelope) => {
            let changed = envelope != original_context;
            let staged = ProviderProtocolContextValue::from_envelope(envelope).unwrap();
            let current_locator_value =
                &persisted.input_payload[CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY];
            let locator = ProviderProtocolContextLocator::parse(current_locator_value)
                .unwrap()
                .expect("the successor locator must parse");
            assert!(
                staged.matches_locator(&locator),
                "current staged context must resolve"
            );
            assert_eq!(staged.matches_locator(&old_locator), !changed);
            assert_eq!(current_locator_value != old_locator_value, changed);
        }
        None => assert!(
            persisted
                .input_payload
                .get(CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY)
                .is_none(),
            "a successor without current context must not retain the predecessor locator"
        ),
    }
    let mut frozen_business = frozen_input.clone();
    let mut successor_business = persisted.input_payload.clone();
    for payload in [&mut frozen_business, &mut successor_business] {
        payload
            .as_object_mut()
            .unwrap()
            .remove(CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY);
        if let Some(sys) = payload.get_mut("sys").and_then(Value::as_object_mut) {
            for key in [
                "native_inference_recovery",
                "public_run_idempotency_fingerprint",
                "public_provider_transport",
            ] {
                sys.remove(key);
            }
            if sys.is_empty() {
                payload.as_object_mut().unwrap().remove("sys");
            }
        }
    }
    assert_eq!(
        successor_business, frozen_business,
        "all frozen business inputs remain intact"
    );
    for _ in 0..2 {
        let PreparedPublishedCallbackResume::Resume { initial_run } =
            f.prepare(&f.command).await.unwrap()
        else {
            panic!("must replay linked successor")
        };
        assert_eq!(initial_run.id, successor.id);
        assert_eq!(
            initial_run.metadata["native_inference_recovery_replay"],
            true
        );
        let replay = f.service.resume_callback(f.command.clone()).await.unwrap();
        assert_eq!(replay.run.id, successor.id);
        assert_eq!(replay.attempt.flow_run_id, f.flow_run_id);
    }
    let extended = appended_context(&f.command);
    assert!(matches!(
        f.prepare(&extended).await.unwrap(),
        PreparedPublishedCallbackResume::StartNewTurnFromHistory
    ));
    assert_eq!(f.repository.flow_run_count(), original_count + 1);
    f.assert_receipt_unchanged();
}

#[tokio::test]
async fn recovery_requires_the_failed_invocations_host_binding() {
    for (field, expected) in [
        (None, "native_recovery_binding_missing"),
        (
            Some("provider_configuration_digest"),
            "native_recovery_configuration_mismatch",
        ),
        (Some("node_id"), "native_recovery_configuration_mismatch"),
        (Some("model"), "native_recovery_configuration_mismatch"),
        (
            Some("artifact_checksum"),
            "native_recovery_configuration_mismatch",
        ),
    ] {
        let mut failure = transport_failure();
        if let Some(field) = field {
            failure["native_inference_binding"][field] = json!("changed");
        } else {
            failure
                .as_object_mut()
                .unwrap()
                .remove("native_inference_binding");
        }
        let fixture = fixture(Some(failure)).await;
        assert_eq!(
            fixture
                .prepare(&fixture.command)
                .await
                .unwrap_err()
                .to_string(),
            format!("conflict: {expected}")
        );
        fixture.assert_receipt_unchanged();
    }
}

#[tokio::test]
async fn changed_configuration_recovery_binds_failed_request_not_previous_callback() {
    let tools = json!([{"type":"function","name":"new_tool","parameters":{"type":"object","properties":{}}}]);
    let f = fixture_with_configuration(Some(transport_failure()), Some(tools)).await;
    assert!(matches!(
        f.prepare(&f.command).await.unwrap(),
        PreparedPublishedCallbackResume::RecoverInference { .. }
    ));
    for tools in [None, Some(json!([]))] {
        let mut command = f.command.clone();
        let mut body = command.native_transport.take().unwrap().into_wire_body();
        if let Some(tools) = tools {
            body["tools"] = tools;
        } else {
            body.as_object_mut().unwrap().remove("tools");
        }
        command.native_transport = Some(ProviderTransportPayload::openai_responses(body).unwrap());
        assert_eq!(
            f.prepare(&command).await.unwrap_err().to_string(),
            "conflict: native_recovery_configuration_mismatch"
        );
    }
    f.assert_receipt_unchanged();
}

#[tokio::test]
async fn malformed_failed_configuration_evidence_cannot_fall_back_to_old_callback() {
    for invalid in [
        Value::Null,
        json!({"digest":"not-a-string"}),
        json!("sha256:forged"),
    ] {
        let mut failure = transport_failure();
        failure["native_inference_configuration_digest"] = invalid;
        let f = fixture(Some(failure)).await;
        assert_eq!(
            f.prepare(&f.command).await.unwrap_err().to_string(),
            "conflict: native_recovery_configuration_mismatch"
        );
        f.assert_receipt_unchanged();
    }
}

#[tokio::test]
async fn completed_callback_configuration_refresh_keeps_receipt_identity() {
    let f = fixture_with_configuration(None, Some(json!([]))).await;
    for tools in [
        json!([]),
        json!([{"type":"function","name":"new_tool","parameters":{"type":"object","properties":{}}}]),
    ] {
        let mut command = f.command.clone();
        let mut body = command.native_transport.take().unwrap().into_wire_body();
        body["tools"] = tools;
        command.native_transport = Some(ProviderTransportPayload::openai_responses(body).unwrap());
        f.service.resume_callback(command).await.unwrap();
        f.assert_receipt_unchanged();
    }
}

fn appended_context(command: &ResumePublishedCallbackCommand) -> ResumePublishedCallbackCommand {
    let mut command = command.clone();
    let mut body = command.native_transport.take().unwrap().into_wire_body();
    body["input"].as_array_mut().unwrap().extend([
        json!({"type":"reasoning","summary":[],"encrypted_content":"new-reasoning"}),
        json!({"role":"assistant","content":"Queued agent context"}),
        json!({"type":"future_context_boundary","opaque":{"preserved":true}}),
    ]);
    command.native_transport = Some(ProviderTransportPayload::openai_responses(body).unwrap());
    command
}

#[tokio::test]
async fn consumed_full_context_extension_starts_new_turn_without_recovery_successor() {
    for failure in [None, Some(transport_failure())] {
        let f = fixture(failure).await;
        let count = f.repository.flow_run_count();
        for refresh in [false, true] {
            let mut command = appended_context(&f.command);
            if refresh {
                let mut body = command.native_transport.take().unwrap().into_wire_body();
                body["tools"] =
                    json!([{"type":"function","name":"fresh","parameters":{"type":"object"}}]);
                body["reasoning"] = json!({"effort":"high"});
                command.native_transport =
                    Some(ProviderTransportPayload::openai_responses(body).unwrap());
            }
            let original_body = command.native_transport.clone().unwrap();
            assert!(matches!(
                f.prepare(&command).await.unwrap(),
                PreparedPublishedCallbackResume::StartNewTurnFromHistory
            ));
            assert_eq!(command.native_transport.as_ref(), Some(&original_body));
        }
        assert_eq!(f.repository.flow_run_count(), count);
        f.assert_receipt_unchanged();
    }
}

#[tokio::test]
async fn consumed_full_context_extension_cannot_bypass_receipt_or_history_identity() {
    let f = fixture(Some(transport_failure())).await;
    for mutation in 0..8 {
        let mut command = appended_context(&f.command);
        let mut body = command.native_transport.take().unwrap().into_wire_body();
        match mutation {
            0 => body["input"][1]["encrypted_content"] = json!("tampered"),
            1 => body["input"][5]["output"] = json!("tampered"),
            2 => body["input"][5]["call_id"] = json!("foreign"),
            3 => body["model"] = json!("foreign"),
            4 => command.response_payload["tool_results"][0]["content"] = json!("tampered"),
            5 => command.source = PublishedCallbackResumeSource::OpenAiChat,
            6 => body["input"]
                .as_array_mut()
                .unwrap()
                .push(json!({"type":"function_call_output","call_id":"call_1","output":"done"})),
            7 => body["input"].as_array_mut().unwrap().push(
                json!({"type":"function_call","call_id":"next","name":"exec","arguments":"{}"}),
            ),
            _ => unreachable!(),
        }
        command.native_transport = Some(ProviderTransportPayload::openai_responses(body).unwrap());
        assert!(f.prepare(&command).await.is_err(), "mutation {mutation}");
    }
    f.assert_receipt_unchanged();
}
