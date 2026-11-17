use super::*;
use crate::openapi_interface::contract::{ApiInput, ApiOperationContext, ApiOutput};
use serde_json::{json, Value};

struct ValueContract;
impl InterfaceContract for ValueContract {
    const CONTRACT_ID: &'static str = "non-create-preparation-fixture";
    const CONTRACT_VERSION: &'static str = "1";
}
impl ApiInput for ValueContract {
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({}))
    }
}
impl ApiOutput for ValueContract {
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({}))
    }
}
struct Identity;
impl ConsoleInterfacePort<ValueContract, ValueContract> for Identity {
    fn execute<'a>(
        &'a self,
        _: &'a UserPrincipal,
        input: ValueContract,
    ) -> ConsoleInterfaceFuture<'a, ValueContract> {
        Box::pin(async move { Ok(input) })
    }
}
#[test]
fn non_create_preparation_accepts_application_and_missing_plans_but_rejects_create() {
    let binding = BindingId::new("http.non-create.fixture.v1").unwrap();
    for operation in ["applications.list", "model_definitions.create"] {
        let registry = compile_registry(
            "api-server.non-create-fixture",
            "api-server.non-create-fixture.graph.v1",
            &[ConsoleInterfaceDeclaration {
                interface_id: operation,
                binding_id: binding.as_str(),
                method: "POST",
                path: "/api/console/fixture",
                mutating: true,
            }],
            Arc::new(Identity),
        )
        .unwrap();
        let result = reject_create_plan(&registry, &binding);
        if operation == "model_definitions.create" {
            assert_eq!(
                result.unwrap_err().0.to_string(),
                "Create binding requires its typed Create input"
            );
        } else {
            assert!(result.is_ok());
        }
        assert!(reject_create_plan(
            &registry,
            &BindingId::new("http.missing.fixture.v1").unwrap()
        )
        .is_ok());
    }
}

#[test]
fn established_principal_producer_envelope_preserves_session_and_has_independent_controls() {
    let binding = BindingId::new("http.non-create.fixture.v1").unwrap();
    let registry = compile_registry(
        "api-server.non-create-fixture",
        "api-server.non-create-fixture.graph.v1",
        &[ConsoleInterfaceDeclaration {
            interface_id: "applications.list",
            binding_id: binding.as_str(),
            method: "POST",
            path: "/api/console/fixture",
            mutating: true,
        }],
        Arc::new(Identity),
    )
    .unwrap();
    let actor =
        domain::ActorContext::scoped(uuid::Uuid::now_v7(), uuid::Uuid::now_v7(), "member", []);
    let principal = UserPrincipal::with_authenticated_session(
        actor.clone(),
        interface_runtime::AuthenticatedSessionIdentity::new("live-owner-session").unwrap(),
    );
    let browser = established_principal_envelope(
        &registry,
        binding.clone(),
        principal.clone(),
        ValueContract,
    )
    .unwrap();
    browser.controls().cancellation().cancel();
    let producer =
        established_principal_envelope(&registry, binding.clone(), principal, ValueContract)
            .unwrap();
    assert_eq!(producer.principal().actor().user_id, actor.user_id);
    assert_eq!(
        producer.principal().actor().current_workspace_id,
        actor.current_workspace_id
    );
    assert_eq!(
        producer.principal().credential_kind(),
        interface_runtime::UserCredentialKind::CookieSession
    );
    assert_eq!(
        producer
            .principal()
            .authenticated_session()
            .unwrap()
            .expose_to_trusted_handler(),
        "live-owner-session"
    );
    assert_eq!(producer.binding_id(), &binding);
    assert_eq!(producer.protocol(), InterfaceProtocol::Http);
    let activated = registry.authentication(&binding).unwrap();
    assert_eq!(producer.authentication_adapter(), activated.adapter());
    assert_eq!(producer.authentication_activation(), activated.activation());
    assert_ne!(
        producer.lineage().invocation_id(),
        browser.lineage().invocation_id()
    );
    assert!(producer.lineage().parent_invocation_id().is_none());
    assert!(!producer.controls().cancellation().is_cancelled());
    assert!(producer.controls().deadline().is_none());
    assert!(established_principal_envelope(
        &registry,
        BindingId::new("http.missing.fixture.v1").unwrap(),
        producer.principal().clone(),
        ValueContract,
    )
    .is_err());
}
