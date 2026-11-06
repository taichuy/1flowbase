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
        "http.runtime.models.metadata.get.v1",
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
    let source = include_str!("../../routes/plugins_and_models/runtime_models.rs");
    assert!(source.contains("interface::invoke(state, headers, method, model_code, uri, body)"));
    assert!(!source.contains("impl interface::RuntimeModelOperationPort for ApiState"));
    assert!(!source.contains("Arc<ApiState> as Arc<dyn interface::RuntimeModelOperationPort>"));
}

#[test]
fn runtime_metadata_has_a_dedicated_typed_contract_and_exact_route() {
    let registry =
        crate::routes::runtime_models::compile_runtime_model_interface_registry_for_test().unwrap();
    let binding = registry
        .binding(&BindingId::new("http.runtime.models.metadata.get.v1").unwrap())
        .unwrap();
    assert_eq!(
        binding.interface_identity().interface_id().as_str(),
        "runtime.models.metadata.get"
    );
    assert_eq!(binding.projection().http_route().unwrap().method(), "GET");
    assert_eq!(
        binding.projection().http_route().unwrap().path(),
        "/api/runtime/models/:model_code/metadata"
    );
    assert_eq!(
        binding.input_contract().contract_id(),
        "runtime-model-metadata-input"
    );
    assert_eq!(
        binding.output_contract().contract_id(),
        "runtime-model-metadata-output"
    );
}

#[test]
fn runtime_metadata_callable_schema_validates_the_success_payload() {
    use crate::routes::runtime_models::metadata::{
        RuntimeDataModelMetadata, RuntimeDataModelRecordCapabilities,
    };
    use serde_json::json;
    use utoipa::OpenApi;

    let spec = serde_json::to_value(crate::openapi::ApiDoc::openapi()).unwrap();
    let operation = crate::openapi_docs::DocsCatalogOperation {
        id: "runtime.models.metadata.get".into(),
        method: "GET".into(),
        path: "/api/runtime/models/{model_code}/metadata".into(),
        summary: None,
        description: None,
        tags: vec![],
        group: "runtime".into(),
        deprecated: false,
    };
    let entry = crate::openapi_interface::catalog_entry_from_operation(&operation, &spec).unwrap();
    let validator = jsonschema::validator_for(&entry.response_schema).unwrap();
    let payload = serde_json::to_value(RuntimeDataModelMetadata {
        model_id: uuid::Uuid::nil(),
        model_code: "orders".into(),
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: uuid::Uuid::nil(),
        record_id_field: Some("id".into()),
        fields: vec![],
        record_capabilities: RuntimeDataModelRecordCapabilities {
            can_list: true,
            can_get: true,
            can_create: false,
            can_update: false,
            can_delete: false,
        },
    })
    .unwrap();
    // The HTTP adapter wraps ApiSuccess, and callable dispatch validates its data payload.
    let response = serde_json::to_value(crate::response::ApiSuccess::new(payload.clone())).unwrap();
    assert!(validator.is_valid(&response["data"]));
    assert!(!validator.is_valid(&response));
    let mut missing_identity = payload.clone();
    missing_identity.as_object_mut().unwrap().remove("model_id");
    assert!(!validator.is_valid(&missing_identity));
    let mut invalid_capability = payload;
    invalid_capability["record_capabilities"]["can_list"] = json!("true");
    assert!(!validator.is_valid(&invalid_capability));
}
