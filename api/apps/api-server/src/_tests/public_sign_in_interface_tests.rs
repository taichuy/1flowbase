use std::sync::Weak;

use interface_runtime::{BindingId, InterfaceExecutionMode, PrincipalProfile};

use crate::{
    app_state::ApiState,
    routes::sign_in_interface::{self, BINDING_ID},
};

#[test]
fn public_sign_in_publishes_one_typed_mutation_plan() {
    let registry = sign_in_interface::compile_registry(Weak::<ApiState>::new())
        .expect("public sign-in catalog must publish");
    let binding_id = BindingId::new(BINDING_ID).expect("fixture binding id is valid");
    let plan = registry
        .plan(&binding_id)
        .expect("public sign-in binding must resolve a plan");

    assert_eq!(registry.bindings().len(), 1);
    assert_eq!(
        plan.definition().execution_mode(),
        InterfaceExecutionMode::Unary
    );
    assert_eq!(
        plan.definition().principal_profile(),
        PrincipalProfile::Public
    );
    assert_eq!(
        plan.definition().target_reference().as_str(),
        "control-plane.auth-kernel.login"
    );
    assert_eq!(
        plan.authentication().activation().as_str(),
        "api-server.public.activation.v1"
    );
}

#[test]
fn public_sign_in_route_projects_cookie_after_terminal_receipt() {
    let source = include_str!("../routes/identity/auth.rs");
    let receipt = source
        .find("outcome.receipt().clone().projected()")
        .expect("route must project the terminal receipt");
    let cookie = source
        .find("Cookie::build")
        .expect("route must preserve cookie projection");

    assert!(
        receipt < cookie,
        "terminal receipt must precede protocol projection"
    );
    assert_eq!(source.matches(".login(").count(), 1);
}
