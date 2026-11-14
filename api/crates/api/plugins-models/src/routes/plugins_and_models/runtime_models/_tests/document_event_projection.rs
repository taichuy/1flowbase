use super::*;
use serde_json::json;

fn snapshot(id: Uuid, operation: &str) -> Arc<AuthorizedOperation> {
    Arc::new(AuthorizedOperation {
        descriptor: DocumentOperationDescriptor {
            operation_id: crate::runtime_data_model_docs::operation_id(id, operation).into(),
            binding_id: BindingId::new("http.runtime.models.dynamic.get.v1").unwrap(),
            interface_identity: InterfaceIdentity::new(
                InterfaceId::new("runtime.models.invoke.get").unwrap(),
                InterfaceVersion::new("1").unwrap(),
            ),
            registration_generation: GENERATION.into(),
            target_identity: format!("model:{id}:{operation}").into(),
            target_generation: format!("snapshot:{id}").into(),
            schemas: DocumentOperationSchemas {
                request: Some(
                    json!({"type":"object","properties":{"path":{"type":"object","properties":{"id":{"type":"string"}}},"query":{"type":"object","properties":{"page":{"type":"integer"},"filter":{"type":"string"}}}}}),
                ),
                result: Some(json!({"type":"object"})),
                event: None,
                request_unavailable: None,
                result_unavailable: None,
            },
        },
        model_id: id,
        model_code: format!("model_{id}"),
        workspace_id: Uuid::now_v7(),
        path_parameters: [("id".into(), "row-actual".into())].into_iter().collect(),
    })
}
#[test]
fn runtime_document_codec_keeps_actual_values_and_excludes_unconsumed_query_carriers() {
    let snapshot = snapshot(Uuid::now_v7(), "list_records");
    let codec = Codec {
        snapshot: snapshot.clone(),
    };
    let input = interface::RuntimeModelOperationInput {
        method: plugin_framework::DataModelOperationMethod::Get,
        model_code: snapshot.model_code.clone(),
        path: "/actual/row".into(),
        query: Some("page=41&filter=actual-long-filter&authorization=SENTINEL".into()),
        body: Vec::new(),
    };
    let projected = codec.project_input(&input).unwrap();
    assert_eq!(
        projected,
        json!({"path":{"id":"row-actual"},"query":{"page":41,"filter":"actual-long-filter"}})
    );
    assert!(!projected.to_string().contains("SENTINEL"));
    let output = interface::RuntimeModelOperationOutput {
        status: interface::RuntimeModelOperationStatus::Ok,
        data: json!({"items":(0..80).collect::<Vec<_>>(),"total":80}),
    };
    assert_eq!(codec.project_output(&output).unwrap(), output.data);
    assert_eq!(output.data["items"].as_array().unwrap().len(), 80);
}
#[test]
fn runtime_document_selector_rejects_other_binding_generation_and_model_identity() {
    let original = snapshot(Uuid::now_v7(), "get_record");
    let selector = Selector {
        interface_id: original
            .descriptor
            .interface_identity
            .interface_id()
            .clone(),
        binding_id: original.descriptor.binding_id.clone(),
    };
    assert!(selector.select(&original).is_ok());
    assert!(selector.select(&"not an authorized snapshot").is_err());
    let mut changed = snapshot(Uuid::now_v7(), "get_record");
    Arc::get_mut(&mut changed).unwrap().descriptor.operation_id =
        original.descriptor.operation_id.clone();
    assert!(selector.select(&changed).is_err());
    let mut changed = snapshot(Uuid::now_v7(), "get_record");
    Arc::get_mut(&mut changed)
        .unwrap()
        .descriptor
        .registration_generation = "stale".into();
    assert!(selector.select(&changed).is_err());
    let mut changed = snapshot(Uuid::now_v7(), "get_record");
    Arc::get_mut(&mut changed).unwrap().descriptor.binding_id =
        BindingId::new("http.runtime.models.dynamic.post.v1").unwrap();
    assert!(selector.select(&changed).is_err());
}
#[test]
fn runtime_document_codec_rejects_another_model_and_invalid_declared_query_type() {
    let snapshot = snapshot(Uuid::now_v7(), "get_record");
    let codec = Codec {
        snapshot: snapshot.clone(),
    };
    let mut input = interface::RuntimeModelOperationInput {
        method: plugin_framework::DataModelOperationMethod::Get,
        model_code: "other_model".into(),
        path: "/actual/row".into(),
        query: None,
        body: Vec::new(),
    };
    assert!(codec.project_input(&input).is_err());
    input.model_code = snapshot.model_code.clone();
    input.query = Some("page=invalid".into());
    assert!(codec.project_input(&input).is_err());
}
