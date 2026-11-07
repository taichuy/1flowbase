use super::*;
use control_plane::application_public_api::native::{NativeRunStatus, NativeUsage};
use interface_runtime::{
    ManagedInterfaceFreezeFuture, ManagedInterfaceFreezeRequest, ManagedInterfaceInvocationFactory,
};

fn run() -> NativeRunResult {
    NativeRunResult {
        id: uuid::Uuid::now_v7(),
        application_id: uuid::Uuid::now_v7(),
        api_key_id: uuid::Uuid::now_v7(),
        publication_version_id: uuid::Uuid::now_v7(),
        status: NativeRunStatus::Succeeded,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: Some("complete answer ".repeat(300)),
        answer_segments: None,
        required_action: None,
        tool_calls: None,
        usage: Some(NativeUsage {
            prompt_tokens: Some(14),
            completion_tokens: Some(9),
            total_tokens: Some(23),
            ..Default::default()
        }),
        error: None,
        operation_terminal: None,
        created_at: time::OffsetDateTime::now_utc(),
    }
}
fn operation(id: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: "/v1/chat/completions".into(),
    }
}

#[test]
fn unary_chat_document_is_the_same_http_result_with_real_round_usage() {
    let callback = uuid::Uuid::now_v7();
    let baseline = openai::usage::ResponseUsageBaseline::Recorded(Some(NativeUsage {
        prompt_tokens: Some(10),
        completion_tokens: Some(4),
        total_tokens: Some(14),
        ..Default::default()
    }));
    let run = run();
    let id = run.id;
    let output = CompatibilityProtocolOutput::project(
        run,
        Some(openai::chat_protocol_projection(
            "actual-model".into(),
            Some(callback),
            Some(baseline),
        )),
    )
    .unwrap();
    let operation = operation("compat.openai.chat-completions.blocking");
    let event = output.project_document_output(&operation).unwrap();
    assert_eq!(event["model"], "actual-model");
    assert_eq!(
        event["choices"][0]["message"]["content"],
        "complete answer ".repeat(300)
    );
    assert_eq!(event["usage"]["prompt_tokens"], 4);
    assert_eq!(event["usage"]["completion_tokens"], 5);
    assert_eq!(event["usage"]["total_tokens"], 9);
    assert_eq!(event["id"],crate::routes::application_public_api::compat_sse::openai_chat_completion_id_from_callback_task(id,callback));
    assert!(jsonschema::validator_for(
        &CompatibilityProtocolOutput::document_output_schema(&operation).unwrap()
    )
    .unwrap()
    .is_valid(&event));
    let (actual_run, http) = output.into_parts();
    assert_eq!(actual_run.id, id);
    assert_eq!(http, event);
}

#[test]
fn unary_anthropic_random_message_identity_is_frozen_once_for_event_and_http() {
    let output = CompatibilityProtocolOutput::project(
        run(),
        Some(anthropic::message_protocol_projection(
            "actual-claude".into(),
        )),
    )
    .unwrap();
    let operation = operation("compat.anthropic.messages.blocking");
    let first = output.project_document_output(&operation).unwrap();
    let second = output.project_document_output(&operation).unwrap();
    assert_eq!(first, second);
    assert_eq!(first["model"], "actual-claude");
    assert_eq!(first["content"][0]["text"], "complete answer ".repeat(300));
    assert!(jsonschema::validator_for(
        &CompatibilityProtocolOutput::document_output_schema(&operation).unwrap()
    )
    .unwrap()
    .is_valid(&first));
    let (_, http) = output.into_parts();
    assert_eq!(http, first);
}

#[test]
fn unary_protocol_projection_failures_keep_the_existing_http_error_and_no_success_value() {
    let mut run = run();
    run.status = NativeRunStatus::Waiting;
    let id = run.id;
    let error = CompatibilityProtocolOutput::project(
        run,
        Some(openai::chat_protocol_projection("model".into(), None, None)),
    )
    .unwrap_err();
    assert_eq!(error.run().unwrap().id, id);
    assert!(matches!(
        error.into_openai(),
        openai::OpenAiRouteError::RequiredAction
    ));
}

#[test]
fn all_compatibility_bindings_have_typed_documents_and_unary_protocol_schemas_are_explicit() {
    let registry = fixture_registry();
    assert_eq!(registry.document_operations().count(), 17);
    for (interface, _, _, _, _) in super::super::bindings() {
        let schema = CompatibilityProtocolOutput::document_output_schema(&operation(interface))
            .expect("every unary declaration has an explicit protocol result schema");
        assert!(schema.is_object());
    }
    let unary = CompatibilityProtocolOutput::document_output_schema(&operation(
        "compat.openai.chat-completions.blocking",
    ))
    .unwrap();
    let stream = CompatibilityBlockingOutput::document_output_schema(&operation(
        "compat.openai.chat-completions.stream",
    ))
    .unwrap();
    assert!(unary["properties"].get("choices").is_some());
    assert!(stream["properties"].get("answer").is_some());
    assert!(stream["properties"].get("api_key_id").is_none());
}

struct WaitingPort;
impl CompatibilityBlockingPort for WaitingPort {
    fn execute<'a>(
        &'a self,
        _: &'a ApplicationPrincipal,
        _: CompatibilityBlockingInput,
        _: crate::routes::application_public_api::compat_sse::CompatibleResumeConsumer,
    ) -> super::super::CompatibilityBlockingFuture<'a> {
        Box::pin(async {
            let mut run = run();
            run.status = NativeRunStatus::Waiting;
            Ok(CompatibilityBlockingOutput(run))
        })
    }
    fn execute_stream<'a>(
        &'a self,
        _: &'a ApplicationPrincipal,
        _: CompatibilityBlockingInput,
        _: crate::routes::application_public_api::compat_sse::CompatibleResumeConsumer,
    ) -> super::super::CompatibilityBlockingStreamFuture<'a> {
        Box::pin(std::future::pending())
    }
}

struct FixtureModelsPort;
impl super::super::models::CompatibilityModelsPort for FixtureModelsPort {
    fn list<'a>(
        &'a self,
        _: &'a ApplicationPrincipal,
        _: super::super::models::CompatibilityModelsInput,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<
                        super::super::models::CompatibilityModelsOutput,
                        CompatibilityBlockingTargetError,
                    >,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async {
            Err(CompatibilityBlockingTargetError(NativeApiError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "fixture_models_unavailable",
                "model execution is outside compatibility document tests",
            )))
        })
    }
}

fn fixture_registry() -> Arc<CompiledInterfaceRegistry> {
    // Keep these ports in the public_api fixture group while exercising the real
    // compiler and typed handlers. Model execution is outside these test cases.
    super::super::compile_registry(Arc::new(WaitingPort), Arc::new(FixtureModelsPort)).unwrap()
}

struct RegistryOnlyManagedFactory;
impl ManagedInterfaceInvocationFactory for RegistryOnlyManagedFactory {
    fn freeze(&self, _: ManagedInterfaceFreezeRequest) -> ManagedInterfaceFreezeFuture<'_> {
        panic!("managed registry publication must not invoke handlers")
    }
}

fn assert_typed_managed_contract<T: InterfaceContract>(registry: &CompiledInterfaceRegistry) {
    let descriptor = registry
        .managed_contracts()
        .find(|descriptor| {
            descriptor.contract.contract_id() == T::CONTRACT_ID
                && descriptor.contract.version() == T::CONTRACT_VERSION
        })
        .expect("the carrier has its own published managed descriptor");
    assert_eq!(descriptor.rust_type, std::any::type_name::<T>());
    assert_eq!(
        descriptor.schema.as_deref(),
        T::managed_projection_schema().as_ref()
    );
}

#[test]
fn compatibility_unary_and_stream_carriers_publish_distinct_typed_managed_contracts() {
    let snapshot = fixture_registry();
    let operations = snapshot
        .definitions()
        .map(|definition| definition.authorization_operation().clone())
        .collect::<std::collections::BTreeSet<_>>();
    let owners = snapshot
        .definitions()
        .map(|definition| definition.owner().clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut compiler =
        RegistryCompiler::new(snapshot.graph_fingerprint().clone(), operations, owners)
            .with_managed_invocations(Arc::new(RegistryOnlyManagedFactory));
    compiler.absorb_snapshot(&snapshot).unwrap();
    // This is the production publication path that rejects equal contract
    // identities backed by different Rust carriers, even if their schemas match.
    let registry = compiler
        .compile()
        .expect("unary protocol carriers and streaming run carriers publish together");
    assert_typed_managed_contract::<CompatibilityProtocolOutput>(&registry);
    assert_typed_managed_contract::<CompatibilityProtocolTargetError>(&registry);
    assert_typed_managed_contract::<CompatibilityBlockingOutput>(&registry);
    assert_typed_managed_contract::<CompatibilityBlockingTargetError>(&registry);
    assert_eq!(registry.document_operations().count(), 17);
}

#[test]
fn distinct_protocol_contracts_preserve_legacy_safe_managed_fields() {
    let run = run();
    let output = CompatibilityProtocolOutput::project(
        run.clone(),
        Some(openai::chat_protocol_projection(
            "actual-model".into(),
            None,
            None,
        )),
    )
    .unwrap();
    assert_eq!(
        CompatibilityProtocolOutput::managed_projection_schema(),
        CompatibilityBlockingOutput::managed_projection_schema()
    );
    assert_eq!(
        output.project_for_managed_hook(),
        CompatibilityBlockingOutput(run).project_for_managed_hook()
    );
    let native_error = || {
        NativeApiError::new(
            axum::http::StatusCode::CONFLICT,
            "fixture_failure",
            "actual failure message",
        )
    };
    let error = CompatibilityProtocolTargetError::from(native_error());
    assert_eq!(
        CompatibilityProtocolTargetError::managed_projection_schema(),
        CompatibilityBlockingTargetError::managed_projection_schema()
    );
    assert_eq!(
        error.project_for_managed_hook(),
        CompatibilityBlockingTargetError(native_error()).project_for_managed_hook()
    );
}

#[tokio::test]
async fn unary_protocol_projection_error_is_a_failed_kernel_completion_with_the_original_error() {
    use interface_runtime::{
        InterfaceInvocationKernel, InterfaceInvocationTerminal, InterfaceProtocol,
        InvocationEnvelope, InvocationId, InvocationLineage,
    };
    let registry = fixture_registry();
    let workspace = uuid::Uuid::now_v7();
    let principal = ApplicationPrincipal::new(
        uuid::Uuid::now_v7(),
        uuid::Uuid::now_v7(),
        workspace,
        domain::ActorContext::scoped(uuid::Uuid::now_v7(), workspace, "member", []),
    )
    .unwrap();
    let binding = BindingId::new(OPENAI_CHAT_BINDING_ID).unwrap();
    let authentication = registry.authentication(&binding).unwrap();
    let envelope = InvocationEnvelope::with_principal(
        InvocationLineage::root(InvocationId::now_v7()),
        binding.clone(),
        InterfaceProtocol::Http,
        authentication.adapter().clone(),
        authentication.activation().clone(),
        principal,
        None,
        CompatibilityBlockingInput {
            response_projection: Some(openai::chat_protocol_projection(
                "actual-model".into(),
                None,
                None,
            )),
            document: super::super::document_projection::ProtocolRequestDocument::chat(
                &json!({"model":"actual-model","messages":[{"role":"user","content":"actual request"}]}),
            ),
            command: CompatibilityInvocationCommand::Start {
                request: serde_json::from_value(json!({"query":"actual request"})).unwrap(),
                protocol: TranslationProtocol::OpenAiChat,
                provider_transport: None,
            },
        },
    );
    let result=InterfaceInvocationKernel::new(Arc::new(super::super::CompatibilityBlockingAuthorization))
        .invoke::<CompatibilityBlockingInput,CompatibilityProtocolOutput,CompatibilityProtocolTargetError>(registry,envelope).await;
    let failure = match result {
        Err(failure) => failure,
        Ok(_) => panic!("protocol projection failure must not complete successfully"),
    };
    assert_eq!(
        failure.receipt().terminal(),
        InterfaceInvocationTerminal::Failed
    );
    let error = super::super::protocol_invocation_error(failure.into_error());
    assert!(error.run().is_some());
    assert!(matches!(
        error.into_openai(),
        openai::OpenAiRouteError::RequiredAction
    ));
}
