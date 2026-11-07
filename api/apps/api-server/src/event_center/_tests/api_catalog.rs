use super::*;
use crate::{
    event_center::api_projection::{self, ApiEventInput, ApiEventOperationContext, ApiEventOutput},
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};
use interface_runtime::*;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct Input {
    record_id: uuid::Uuid,
    keyword: String,
}
impl InterfaceContract for Input {
    const CONTRACT_ID: &'static str = "api-event-fixture-input";
    const CONTRACT_VERSION: &'static str = "1";
}
impl ApiEventInput for Input {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({"path":{"record_id":self.record_id},"query":{"keyword":self.keyword}}))
    }
}
#[derive(Debug)]
struct Output(String);
impl InterfaceContract for Output {
    const CONTRACT_ID: &'static str = "api-event-fixture-output";
    const CONTRACT_VERSION: &'static str = "1";
}
impl ApiEventOutput for Output {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({"answer":self.0}))
    }
}
struct Port;
impl ConsoleInterfacePort<Input, Output> for Port {
    fn execute<'a>(
        &'a self,
        _: &'a UserPrincipal,
        input: Input,
    ) -> ConsoleInterfaceFuture<'a, Output> {
        Box::pin(async move {
            if input.keyword == "fail" {
                return Err(ConsoleInterfaceTargetError(
                    control_plane::errors::ControlPlaneError::InvalidInput(
                        "controlled-target-failure",
                    )
                    .into(),
                ));
            }
            Ok(Output(format!("actual:{}", input.keyword)))
        })
    }
}
const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "fixture.canonical",
        binding_id: "http.fixture.primary",
        method: "GET",
        path: "/api/console/event-fixture/primary/:record_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "fixture.canonical",
        binding_id: "http.fixture.alias",
        method: "GET",
        path: "/api/console/event-fixture/alias/:record_id",
        mutating: false,
    },
];
fn family() -> Arc<CompiledInterfaceRegistry> {
    console_interface::compile_registry(
        "api-server.event-fixture",
        "graph:event-fixture",
        DECLARATIONS,
        Arc::new(Port),
    )
    .unwrap()
}
fn docs() -> crate::openapi_docs::ApiDocsRegistry {
    let operation = |id: &str| json!({"operationId":id,"summary":id,"parameters":[{"in":"path","name":"record_id","required":true,"schema":{"type":"string","format":"uuid"}},{"in":"query","name":"keyword","schema":{"type":"string"}},{"in":"header","name":"x-credential-sentinel","schema":{"type":"string"}}],"responses":{"200":{"description":"actual result","content":{"application/json":{"schema":{"$ref":"#/components/schemas/ActualOutput"}}}}}});
    crate::openapi_docs::build_api_docs_registry(json!({"openapi":"3.1.0","info":{"title":"event fixture","version":"1"},"paths":{
        "/api/console/event-fixture/primary/{record_id}":{"get":operation("document.primary")},
        "/api/console/event-fixture/alias/{record_id}":{"get":operation("document.alias")}},
        "components":{"schemas":{"ActualOutput":{"type":"object","properties":{"answer":{"type":"string"}},"required":["answer"],"additionalProperties":false}}}})).unwrap()
}
fn entries(docs: &crate::openapi_docs::ApiDocsRegistry) -> Vec<OpenApiCapabilityCatalogEntry> {
    docs.catalog()
        .categories
        .iter()
        .flat_map(|category| &docs.category_operations(&category.id).unwrap().operations)
        .map(|operation| OpenApiCapabilityCatalogEntry {
            interface: crate::openapi_interface::catalog_entry_from_view(
                operation,
                &docs.operation_spec_view(&operation.id).unwrap(),
            )
            .unwrap(),
            source: OpenApiCapabilitySource::StaticApiDocs,
            risk_level: "low",
            bindable: false,
            disabled_reason: Some("mcp-only-restriction"),
            activated_operation: None,
        })
        .collect()
}
#[derive(Default)]
struct Observer(Mutex<Vec<DocumentOperationEvent>>);
impl DocumentOperationObserver for Observer {
    fn observe(&self, event: DocumentOperationEvent) -> DocumentOperationObserverFuture<'_> {
        self.0.lock().unwrap().push(event);
        Box::pin(async { Ok(()) })
    }
}
fn compiled(
    docs: &crate::openapi_docs::ApiDocsRegistry,
    observer: Arc<Observer>,
) -> Arc<CompiledInterfaceRegistry> {
    let family = family();
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:assembled-event-fixture").unwrap(),
        family
            .definitions()
            .map(|definition| definition.authorization_operation().clone()),
        family
            .definitions()
            .map(|definition| definition.owner().clone()),
    )
    .with_document_operation_observer(observer);
    compiler.absorb_snapshot(&family).unwrap();
    api_projection::refine_static_api_events(&mut compiler, docs, &[family]).unwrap();
    compiler.compile().unwrap()
}
struct Authorization;
impl InterfaceAuthorizationPort for Authorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("api-server.console.compiled-operation").unwrap()
    }
    fn authorize(&self, _: InterfaceAuthorizationRequest) -> InterfaceAuthorizationFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}
struct Admission;
impl InterfaceTargetAdmissionPort for Admission {
    fn adapter_reference(&self) -> AdmissionAdapterReference {
        AdmissionAdapterReference::new("api-server.console.protocol-admission").unwrap()
    }
    fn admit(&self, _: InterfaceTargetAdmissionRequest) -> InterfaceTargetAdmissionFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}
fn actor() -> ActorContext {
    ActorContext::root(uuid::Uuid::now_v7(), uuid::Uuid::now_v7(), "root")
}

#[tokio::test]
async fn shared_document_identity_is_refined_by_exact_binding_and_actual_values_are_observed() {
    let docs = docs();
    let observer = Arc::new(Observer::default());
    let snapshot = compiled(&docs, observer.clone());
    validate_static_inventory(&snapshot, &docs).unwrap();
    let kernel = InterfaceInvocationKernel::with_target_admission(
        Arc::new(Authorization),
        Arc::new(Admission),
    );
    for binding in ["http.fixture.primary", "http.fixture.alias"] {
        let binding_id = BindingId::new(binding).unwrap();
        let plan = snapshot.plan(&binding_id).unwrap();
        let record_id = uuid::Uuid::now_v7();
        let request = InvocationEnvelope::with_principal(
            InvocationLineage::root(InvocationId::now_v7()),
            binding_id,
            InterfaceProtocol::Http,
            plan.authentication().adapter().clone(),
            plan.authentication().activation().clone(),
            UserPrincipal::server_delegation(actor()),
            None,
            Input {
                record_id,
                keyword: "actual-value".into(),
            },
        );
        let result = kernel
            .invoke::<Input, Output, ConsoleInterfaceTargetError>(snapshot.clone(), request)
            .await
            .unwrap();
        assert_eq!(result.value().0, "actual:actual-value");
    }
    let events = observer.0.lock().unwrap();
    assert_eq!(events.len(), 4);
    assert_eq!(
        events[0].descriptor().operation_id.as_ref(),
        "document.primary"
    );
    assert_eq!(
        events[2].descriptor().operation_id.as_ref(),
        "document.alias"
    );
    let DocumentOperationEvent::Before { input, .. } = &events[0] else {
        panic!("Before")
    };
    assert_eq!(input["query"]["keyword"], "actual-value");
    assert!(input.get("headers").is_none());
    let (_, _, payload) = crate::event_center::api_observer::envelope(&events[1]);
    assert_eq!(payload["operation_id"], "document.primary");
    assert!(payload.get("interface_id").is_none());
    assert_eq!(payload["output"]["answer"], "actual:actual-value");
}
#[test]
fn full_static_rows_remain_discoverable_independently_of_mcp_bindability_and_details_are_lazy() {
    let docs = docs();
    let snapshot = compiled(&docs, Arc::new(Observer::default()));
    let entries = entries(&docs);
    assert_eq!(entries.len(), 2);
    for entry in entries {
        assert!(!entry.bindable);
        let summary = project_entry(&entry, Some(&snapshot), &actor(), None, false);
        assert!(summary.operation.before.available() && summary.operation.completion.available());
        assert!(
            summary.before_schema.is_none()
                && summary.completion_schema.is_none()
                && summary.request_schema.is_none()
                && summary.result_schema.is_none()
        );
        let detail = details_for_entry(&entry, Some(&snapshot), &actor(), None);
        let request = detail.request_schema.unwrap();
        assert!(request["properties"].get("headers").is_none());
        assert!(detail.before_schema.unwrap()["properties"]
            .get("operation_id")
            .is_some());
        assert_eq!(
            detail.result_schema.unwrap()["properties"]["answer"]["type"],
            "string"
        );
    }
}
#[test]
fn inventory_rejects_unresolved_inheritance_and_missing_document_contract_is_explicit() {
    let docs = docs();
    let family = family();
    assert!(validate_static_inventory(&family, &docs).is_err());
    let empty = crate::openapi_docs::build_api_docs_registry(
        json!({"openapi":"3.1.0","info":{"title":"empty","version":"1"},"paths":{}}),
    )
    .unwrap();
    let snapshot = compiled(&empty, Arc::new(Observer::default()));
    for capability in snapshot.document_operations() {
        let desc = capability.static_descriptor.unwrap();
        assert!(desc.schemas.request.is_none() && desc.schemas.result.is_none());
        assert_eq!(
            desc.schemas.request_unavailable.as_deref(),
            Some("missing_openapi_contract")
        );
    }
}
#[test]
fn ambiguous_document_route_correspondence_rejects_publication() {
    let source = family();
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:ambiguous").unwrap(),
        source
            .definitions()
            .map(|d| d.authorization_operation().clone()),
        source.definitions().map(|d| d.owner().clone()),
    );
    compiler.absorb_snapshot(&source).unwrap();
    let docs=crate::openapi_docs::build_api_docs_registry(json!({"openapi":"3.1.0","info":{"title":"ambiguous","version":"1"},"paths":{
        "/api/console/event-fixture/primary/{record_id}":{"get":{"operationId":"one","responses":{"200":{"description":"ok"}}}},
        "/api/console/event-fixture/primary/{other_id}":{"get":{"operationId":"two","responses":{"200":{"description":"ok"}}}}}})).unwrap();
    assert!(api_projection::refine_static_api_events(&mut compiler, &docs, &[source]).is_err());
}

#[tokio::test]
async fn failed_typed_target_publishes_only_real_failure_classification_without_success_output() {
    let docs = docs();
    let observer = Arc::new(Observer::default());
    let snapshot = compiled(&docs, observer.clone());
    let binding = BindingId::new("http.fixture.primary").unwrap();
    let plan = snapshot.plan(&binding).unwrap();
    let request = InvocationEnvelope::with_principal(
        InvocationLineage::root(InvocationId::now_v7()),
        binding,
        InterfaceProtocol::Http,
        plan.authentication().adapter().clone(),
        plan.authentication().activation().clone(),
        UserPrincipal::server_delegation(actor()),
        None,
        Input {
            record_id: uuid::Uuid::now_v7(),
            keyword: "fail".into(),
        },
    );
    let kernel = InterfaceInvocationKernel::with_target_admission(
        Arc::new(Authorization),
        Arc::new(Admission),
    );
    let failure = kernel
        .invoke::<Input, Output, ConsoleInterfaceTargetError>(snapshot, request)
        .await
        .unwrap_err();
    assert_eq!(
        failure.receipt().terminal(),
        InterfaceInvocationTerminal::Failed
    );
    let events = observer.0.lock().unwrap();
    assert_eq!(events.len(), 2);
    let (phase, terminal, payload) = crate::event_center::api_observer::envelope(&events[1]);
    assert_eq!(phase, InterfaceEventPhase::Completion);
    assert_eq!(terminal, Some(InterfaceEventTerminal::Failed));
    assert_eq!(payload["terminal"], "failed");
    assert!(payload.get("output").is_none());
    assert!(payload.get("input").is_none());
    assert!(payload.get("classification").is_some());
    assert!(
        InterfaceEventBinding::api("document.primary", phase).matches_operation(
            "document.primary",
            phase,
            terminal
        )
    );
    assert!(
        !InterfaceEventBinding::legacy("document.primary", phase).matches_operation(
            "document.primary",
            phase,
            terminal
        )
    );
}

#[test]
fn explicit_owner_restriction_changes_only_its_phase_and_unknown_owner_code_fails_closed() {
    let docs = docs();
    let source = compiled(&docs, Arc::new(Observer::default()));
    for code in ["binary_response", "future-owner-reason"] {
        let mut compiler = RegistryCompiler::new(
            GraphFingerprint::new("graph:phase-restriction").unwrap(),
            source
                .definitions()
                .map(|d| d.authorization_operation().clone()),
            source.definitions().map(|d| d.owner().clone()),
        );
        compiler.absorb_snapshot(&source).unwrap();
        let binding = BindingId::new("http.fixture.primary").unwrap();
        let mut descriptor = source
            .document_operation(&binding)
            .unwrap()
            .static_descriptor
            .unwrap()
            .clone();
        descriptor.schemas.result = None;
        descriptor.schemas.result_unavailable = Some(Arc::from(code));
        compiler
            .rebind_document_operation_descriptor(&binding, descriptor)
            .unwrap();
        let snapshot = compiler.compile().unwrap();
        let entry = entries(&docs)
            .into_iter()
            .find(|entry| entry.interface.operation_id == "document.primary")
            .unwrap();
        let detail = details_for_entry(&entry, Some(&snapshot), &actor(), None);
        assert!(detail.operation.before.available());
        assert_eq!(
            detail.operation.completion,
            unavailable(if code == "binary_response" {
                ApiEventUnavailableReason::BinaryResponse
            } else {
                ApiEventUnavailableReason::TargetUnavailable
            })
        );
        assert!(detail.completion_schema.is_none() && detail.result_schema.is_none());
    }
}
