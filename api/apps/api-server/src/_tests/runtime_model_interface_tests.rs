use interface_runtime::{BindingId, InterfaceExecutionMode, InterfaceProtocol};

#[test]
fn eil_f07_dynamic_runtime_methods_publish_frozen_typed_bindings() {
    let registry =
        crate::routes::runtime_models::compile_runtime_model_interface_registry_for_test()
            .expect("dynamic runtime model interface registry must compile");
    let expected = [
        "http.runtime.models.dynamic.get.v1",
        "http.runtime.models.dynamic.post.v1",
        "http.runtime.models.dynamic.put.v1",
        "http.runtime.models.dynamic.patch.v1",
        "http.runtime.models.dynamic.delete.v1",
    ];

    assert_eq!(registry.bindings().len(), expected.len());
    for raw_binding_id in expected {
        let binding_id = BindingId::new(raw_binding_id).expect("fixture binding id is valid");
        let plan = registry
            .plan(&binding_id)
            .expect("dynamic runtime binding must resolve a frozen plan");
        assert_eq!(
            plan.binding().projection().protocol(),
            InterfaceProtocol::Http
        );
        assert_eq!(
            plan.definition().execution_mode(),
            InterfaceExecutionMode::Unary
        );
        assert_eq!(
            plan.authentication().adapter().as_str(),
            "api-server.runtime-user"
        );
        assert_eq!(
            plan.authentication().activation().as_str(),
            "api-server.runtime-user.activation.v1"
        );
        assert_eq!(
            plan.effective_handler().handler(),
            plan.definition().handler_reference()
        );
    }
}

#[test]
fn eil_f07_dynamic_runtime_router_has_one_kernel_execution_path() {
    let source = include_str!("../routes/plugins_and_models/runtime_models.rs");
    assert!(source.contains("interface::invoke(state, headers, method, model_code, uri, body)"));
    assert!(!source.contains("impl interface::RuntimeModelOperationPort for ApiState"));
    assert!(!source.contains("Arc<ApiState> as Arc<dyn interface::RuntimeModelOperationPort>"));
}
