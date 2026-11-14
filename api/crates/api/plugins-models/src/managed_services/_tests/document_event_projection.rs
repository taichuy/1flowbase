use super::*;
use crate::managed_services::{
    interface::{ManagedServiceInput, ManagedServiceOutput},
    stream::{StreamEvent, StreamInput},
};
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleServerStreamFuture, ConsoleServerStreamPort,
};

fn operation(streaming: bool) -> plugin_framework::ManagedServiceOperation {
    serde_json::from_value(json!({
        "interface_id":"fixture.document","contribution_id":"fixture.document","method":"POST",
        "path":"/api/console/managed-services/fixture/items/{item_id}","summary":"Actual items","description":"Actual typed fixture operation",
        "input_schema":{"type":"object","properties":{
            "path":{"type":"object","properties":{"item_id":{"type":"string"}},"required":["item_id"]},
            "query":{"type":"object","properties":{"locale":{"type":"string"}}},
            "body":{"type":"object","properties":{
                "question":{"type":"string"},"password":{"type":"string"},
                "items":{"type":"array","items":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}},
                "credential":{"type":"string","writeOnly":true,"default":"SCHEMA_MANAGED_SECRET"},
                "outbound":{"type":"string","format":"password"},"keyMaterial":{"type":"string","secret":true}
            },"required":["question","items","credential","outbound","keyMaterial"]}
        },"required":["path","query","body"]},
        "output_schema":{"type":"object","properties":{"items":{"type":"array","items":{"type":"string"}},"outputSecret":{"type":"string","secret":true}},"required":["items","outputSecret"]},
        "stream":streaming.then(||json!({"input_schema":{"type":"object","properties":{"input":{"type":"string"}}},"event_schema":{"type":"object","properties":{"chunk":{"type":"string"}},"required":["chunk"]}}))
    })).unwrap()
}

fn context(operation: &plugin_framework::ManagedServiceOperation) -> ApiOperationContext {
    ApiOperationContext {
        interface_id: operation.interface_id.clone(),
        method: operation.method.clone(),
        path: operation.path.clone(),
    }
}

fn payload() -> Value {
    json!({"path":{"item_id":"actual-id"},"query":{"locale":"zh-Hans"},"headers":{"authorization":"TRANSPORT_MANAGED_SECRET"},"body":{
        "question":"original question","password":"ordinary documented business field",
        "items":(0..40).map(|index|json!({"text":format!("actual item {index}")})).collect::<Vec<_>>(),
        "credential":"WRITEONLY_MANAGED_SECRET","outbound":"FORMAT_MANAGED_SECRET","keyMaterial":"MARKER_MANAGED_SECRET","unknown":"UNKNOWN_MANAGED_SECRET"
    }})
}

#[test]
fn managed_document_codec_uses_actual_post_before_json_and_explicit_owner_markers() {
    let operation = operation(false);
    let policy = Arc::new(OperationPolicy::new(&operation));
    let mut input = ManagedServiceInput {
        interface_id: operation.interface_id.clone(),
        payload: payload(),
        stream: None,
        event_policy: policy.clone(),
    };
    input.payload["body"]["question"] = json!("post-Before actual question");
    let value = input.project_document_input(&context(&operation)).unwrap();
    assert_eq!(value["body"]["question"], "post-Before actual question");
    assert_eq!(
        value["body"]["password"], "ordinary documented business field",
        "property names alone do not classify credentials"
    );
    assert_eq!(value["body"]["items"].as_array().unwrap().len(), 40);
    assert_eq!(value["body"]["items"][39]["text"], "actual item 39");
    for sentinel in [
        "TRANSPORT_MANAGED_SECRET",
        "WRITEONLY_MANAGED_SECRET",
        "FORMAT_MANAGED_SECRET",
        "MARKER_MANAGED_SECRET",
        "UNKNOWN_MANAGED_SECRET",
    ] {
        assert!(!value.to_string().contains(sentinel));
    }
    let schema = policy.request_schema.as_ref().unwrap();
    assert!(!schema.to_string().contains("SCHEMA_MANAGED_SECRET"));
    for property in ["credential", "outbound", "keyMaterial"] {
        assert!(schema
            .pointer(&format!("/properties/body/properties/{property}"))
            .is_none());
    }
    let validator = jsonschema::validator_for(schema).unwrap();
    assert!(validator.is_valid(&value));
    assert!(!validator.is_valid(&input.payload));
    assert!(value.get("body_bytes").is_none());
}

#[test]
fn managed_document_ticket_and_true_business_terminal_share_only_safe_result_schema() {
    let operation = operation(true);
    let policy = Arc::new(OperationPolicy::new(&operation));
    let context = context(&operation);
    let ticket = ManagedServiceOutput(
        json!({"ticket":"TICKET_MANAGED_SECRET"}),
        OutputPolicy::TicketIssued(policy.clone()),
    );
    assert_eq!(
        ticket.0["ticket"], "TICKET_MANAGED_SECRET",
        "HTTP owner retains its actual ticket response"
    );
    let safe_ticket = ticket.project_document_output(&context).unwrap();
    assert_eq!(safe_ticket, json!({}));
    let items = (0..40)
        .map(|index| format!("actual terminal item {index}"))
        .collect::<Vec<_>>();
    let result = ManagedServiceOutput(
        json!({"items":items,"outputSecret":"OUTPUT_MANAGED_SECRET"}),
        OutputPolicy::BusinessResult(policy.clone()),
    );
    let value = result.project_document_output(&context).unwrap();
    assert_eq!(value, json!({"items":items}));
    assert_eq!(result.0["outputSecret"], "OUTPUT_MANAGED_SECRET");
    assert!(value.get("stream_completed").is_none());
    let schema = policy.result_schema().unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.is_valid(&safe_ticket));
    assert!(validator.is_valid(&value));
    assert!(!validator.is_valid(&json!({"ticket":"TICKET_MANAGED_SECRET"})));
    assert!(!validator.is_valid(&json!({"chunk":"this is a frame, not the terminal"})));
}

#[test]
fn managed_document_duplex_projection_never_serializes_handles_origin_or_ack_owner() {
    use runtime_core::runtime_backend::{RuntimeManagedDuplex, RuntimeManagedDuplexEvent};
    let operation = operation(true);
    let policy = Arc::new(OperationPolicy::new(&operation));
    let (_input, receiver) = tokio::sync::mpsc::channel(1);
    let (events, _receiver) = tokio::sync::mpsc::channel(1);
    let (_cancel, cancel) = tokio::sync::watch::channel(false);
    let mut input = ManagedServiceInput {
        interface_id: operation.interface_id.clone(),
        payload: payload(),
        event_policy: policy,
        stream: Some(StreamInput::Run {
            duplex: RuntimeManagedDuplex {
                input: Arc::new(tokio::sync::Mutex::new(receiver)),
                events,
            },
            cancel,
        }),
    };
    let context = context(&operation);
    let run_value = input.project_document_input(&context).unwrap();
    input.stream = Some(StreamInput::Ticket {
        origin: "ORIGIN_MANAGED_SECRET".into(),
    });
    let ticket_value = input.project_document_input(&context).unwrap();
    assert_eq!(run_value, ticket_value);
    assert!(!ticket_value.to_string().contains("ORIGIN_MANAGED_SECRET"));
    assert!(ticket_value.get("stream").is_none());
    let (ack, mut consumed) = tokio::sync::oneshot::channel();
    let frame = StreamEvent(RuntimeManagedDuplexEvent {
        event: json!({"chunk":"actual frame"}),
        consumed: ack,
    });
    assert!(frame.project_document_output(&context).is_err());
    assert!(
        matches!(
            consumed.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ),
        "projection must not consume subscriber ACK credit"
    );
}

#[test]
fn managed_document_plain_json_and_typed_maps_preserve_actual_business_values() {
    let mut operation = operation(false);
    operation.input_schema =
        json!({"type":"object","properties":{"body":true},"required":["body"]});
    operation.output_schema =
        json!({"type":"object","additionalProperties":{"type":"array","items":{"type":"string"}}});
    let policy = Arc::new(OperationPolicy::new(&operation));
    let source = json!({"body":{"ordinary":[1,{"arbitrary":"business JSON"}],"nested":{"body":{"text":"full value"}}}});
    let input = ManagedServiceInput {
        interface_id: operation.interface_id.clone(),
        payload: source.clone(),
        stream: None,
        event_policy: policy.clone(),
    };
    assert_eq!(
        input.project_document_input(&context(&operation)).unwrap(),
        source
    );
    assert_eq!(
        policy
            .request_schema
            .as_ref()
            .unwrap()
            .pointer("/properties/body"),
        Some(&Value::Bool(true))
    );
    let source =
        json!({"actual-key":(0..40).map(|index|format!("value-{index}")).collect::<Vec<_>>()});
    let output = ManagedServiceOutput(source.clone(), OutputPolicy::BusinessResult(policy.clone()));
    assert_eq!(
        output
            .project_document_output(&context(&operation))
            .unwrap(),
        source
    );
    assert!(jsonschema::validator_for(&policy.result_schema().unwrap())
        .unwrap()
        .is_valid(&source));
}

#[test]
fn managed_document_request_uses_actual_http_locations_instead_of_worker_envelope() {
    let mut operation = operation(false);
    operation.method = "GET".into();
    let policy = Arc::new(OperationPolicy::new(&operation));
    let input = ManagedServiceInput {
        interface_id: operation.interface_id.clone(),
        payload: payload(),
        stream: None,
        event_policy: policy.clone(),
    };
    let value = input.project_document_input(&context(&operation)).unwrap();
    assert_eq!(
        value,
        json!({"path":{"item_id":"actual-id"},"query":{"locale":"zh-Hans"}})
    );
    let schema = policy.request_schema.as_ref().unwrap();
    assert!(
        schema.pointer("/properties/body").is_none(),
        "GET has no documented HTTP request body even though its worker envelope has body"
    );
    assert!(jsonschema::validator_for(schema).unwrap().is_valid(&value));
}

struct PendingPort;
impl ConsoleInterfacePort<ManagedServiceInput, ManagedServiceOutput> for PendingPort {
    fn execute<'a>(
        &'a self,
        _principal: &'a interface_runtime::UserPrincipal,
        _input: ManagedServiceInput,
    ) -> ConsoleInterfaceFuture<'a, ManagedServiceOutput> {
        Box::pin(std::future::pending())
    }
}
impl ConsoleServerStreamPort<ManagedServiceInput, StreamEvent, ManagedServiceOutput>
    for PendingPort
{
    fn execute_stream(
        &self,
        _principal: &interface_runtime::UserPrincipal,
        _input: ManagedServiceInput,
    ) -> ConsoleServerStreamFuture<StreamEvent, ManagedServiceOutput> {
        Box::pin(std::future::pending())
    }
}

fn compiled(
    operation: &plugin_framework::ManagedServiceOperation,
) -> Arc<CompiledInterfaceRegistry> {
    let graph = "managed-document-fixture-v1";
    let binding_id = format!("http.{}.v1", operation.interface_id);
    let declarations = [ConsoleInterfaceDeclaration {
        interface_id: &operation.interface_id,
        binding_id: &binding_id,
        method: &operation.method,
        path: &operation.path,
        mutating: true,
    }];
    let registry = if operation.stream.is_some() {
        console_interface::compile_server_stream_registry_with_scope(
            "fixture",
            graph,
            &declarations,
            Arc::new(PendingPort),
            interface_runtime::InterfaceScope::System,
        )
    } else {
        console_interface::compile_registry_with_scope(
            "fixture",
            graph,
            &declarations,
            Arc::new(PendingPort),
            interface_runtime::InterfaceScope::System,
        )
    }
    .unwrap();
    let mut compiler = RegistryCompiler::new(
        interface_runtime::GraphFingerprint::new(graph).unwrap(),
        [interface_runtime::AuthorizationOperation::new(&operation.interface_id).unwrap()],
        [interface_runtime::InterfaceOwner::new("fixture").unwrap()],
    );
    compiler.absorb_snapshot(&registry).unwrap();
    let policies = BTreeMap::from([(
        operation.interface_id.clone(),
        Arc::new(OperationPolicy::new(operation)),
    )]);
    refine_registry(&mut compiler, &registry, &policies).unwrap();
    compiler.compile().unwrap()
}

#[test]
fn managed_document_registry_freezes_real_manifest_schemas_and_stream_terminal() {
    for streaming in [false, true] {
        let operation = operation(streaming);
        let registry = compiled(&operation);
        assert_eq!(
            registry.document_operations().count(),
            1,
            "generic codec is registered exactly once"
        );
        let binding =
            interface_runtime::BindingId::new(format!("http.{}.v1", operation.interface_id))
                .unwrap();
        let descriptor = registry
            .document_operation(&binding)
            .unwrap()
            .static_descriptor
            .unwrap();
        let policy = OperationPolicy::new(&operation);
        assert_eq!(descriptor.schemas.request, policy.request_schema);
        assert_eq!(descriptor.schemas.result, policy.result_schema());
        assert!(descriptor.schemas.request_unavailable.is_none());
        assert!(descriptor.schemas.result_unavailable.is_none());
        assert!(
            descriptor.schemas.event.is_none(),
            "frame schema must never masquerade as the terminal schema"
        );
        assert_eq!(registry.bindings().count(), 1);
    }
}

#[test]
fn managed_document_sensitive_root_has_explicit_reason_and_policy_binding_mismatch_fails_closed() {
    let mut operation = operation(false);
    operation.output_schema = json!({"type":"string","format":"password"});
    let registry = compiled(&operation);
    let binding =
        interface_runtime::BindingId::new(format!("http.{}.v1", operation.interface_id)).unwrap();
    let descriptor = registry
        .document_operation(&binding)
        .unwrap()
        .static_descriptor
        .unwrap();
    assert!(descriptor.schemas.result.is_none());
    assert_eq!(
        descriptor.schemas.result_unavailable.as_deref(),
        Some("credential_boundary")
    );
    let policy = Arc::new(OperationPolicy::new(&operation));
    let output = ManagedServiceOutput(
        json!("ROOT_MANAGED_SECRET"),
        OutputPolicy::BusinessResult(policy.clone()),
    );
    assert!(output
        .project_document_output(&context(&operation))
        .is_err());
    let input = ManagedServiceInput {
        interface_id: operation.interface_id.clone(),
        payload: payload(),
        stream: None,
        event_policy: policy,
    };
    let mut wrong = context(&operation);
    wrong.interface_id = "fixture.other".into();
    assert!(input.project_document_input(&wrong).is_err());
}
