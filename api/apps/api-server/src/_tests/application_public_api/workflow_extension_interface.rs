use std::sync::Arc;

use interface_runtime::{BindingId, InterfaceExecutionMode, PrincipalProfile, ProtocolProjection};

use crate::routes::application_public_api::workflow_extension_interface::{
    self, WorkflowExtensionFuture, WorkflowExtensionInput, WorkflowExtensionPort,
};

struct UnavailableWorkflowExtension;

impl WorkflowExtensionPort for UnavailableWorkflowExtension {
    fn invoke<'a>(
        &'a self,
        _actor: &'a domain::ActorContext,
        _principal: control_plane::application_public_api::workflow_extension::WorkflowHttpPrincipal,
        _input: WorkflowExtensionInput,
    ) -> WorkflowExtensionFuture<'a> {
        Box::pin(async { panic!("catalog fixture must not execute the workflow extension") })
    }
}

#[test]
fn workflow_extension_publishes_one_typed_user_plan() {
    let registry =
        workflow_extension_interface::compile_registry(Arc::new(UnavailableWorkflowExtension))
            .expect("workflow extension catalog must publish");
    let binding_id = BindingId::new(workflow_extension_interface::BINDING_ID)
        .expect("fixture binding id is valid");
    let plan = registry
        .plan(&binding_id)
        .expect("workflow extension binding must resolve a plan");

    assert_eq!(registry.bindings().len(), 1);
    assert_eq!(
        plan.definition().execution_mode(),
        InterfaceExecutionMode::Unary
    );
    assert_eq!(
        plan.definition().principal_profile(),
        PrincipalProfile::User
    );
    assert_eq!(
        plan.definition().target_reference().as_str(),
        "control-plane.workflow-extension-run"
    );
    assert_eq!(
        plan.authentication().activation().as_str(),
        "api-server.console.require-session.activation.v1"
    );
    assert!(matches!(
        plan.binding().projection(),
        ProtocolProjection::Http(route)
            if route.method() == "ANY" && route.path() == "/api/ex/*slug"
    ));
}

#[test]
fn workflow_extension_route_has_one_frozen_authentication_and_execution_owner() {
    let source = include_str!("../../routes/application_public_api/ex.rs");

    assert_eq!(source.matches(".authenticate(").count(), 1);
    assert!(!source.contains("require_session(&state"));
    assert!(!source.contains("require_csrf(&headers"));
    assert_eq!(
        source
            .matches("WorkflowExtensionRunService::new(dependencies.store.clone())")
            .count(),
        1
    );
    assert!(!source.contains("WorkflowExtensionRunService::new(state.store.clone())"));
    assert_eq!(
        source.matches(".invoke::<WorkflowExtensionInput").count(),
        1
    );
}

#[test]
fn workflow_extension_projects_only_after_terminal_receipt() {
    let source = include_str!("../../routes/application_public_api/ex.rs");
    let receipt = source
        .find("outcome.receipt().clone().projected()")
        .expect("terminal receipt must be projected");
    let projection = source
        .find("project_workflow_extension_output(outcome.into_value())")
        .expect("protocol output must be projected");

    assert!(receipt < projection);
}
