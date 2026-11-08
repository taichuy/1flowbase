use super::*;
#[test]
fn model_routing_parameters_preserve_typed_distribution_configuration_and_full_target_lists() {
    let op = ApiOperationContext {
        interface_id: "model_providers.main_instance.update".into(),
        method: "PUT".into(),
        path: "/api/console/settings/model-providers/providers/:provider_code/main-instance".into(),
    };
    let config = std::collections::BTreeMap::from([
        (
            "weight".into(),
            ModelProviderDistributionConfigValueBody::Integer(80),
        ),
        (
            "label".into(),
            ModelProviderDistributionConfigValueBody::String("Public business value".into()),
        ),
    ]);
    let body = UpdateModelProviderMainInstanceBody {
        auto_include_new_instances: true,
        expected_revision: 7,
        model_routing_policies: Some(vec![ModelProviderMainModelRoutingPolicyBody {
            model_id: "model-a".into(),
            distribution_rule: "round_robin".into(),
            distribution_rule_contract_version: Some("1".into()),
            distribution_rule_config: config,
            provider_instance_ids: vec![uuid::Uuid::nil(); 90],
            excluded_provider_instance_ids: vec![],
        }]),
    };
    let expected = serde_json::to_value(&body).unwrap();
    let value = ProviderRoutingInput::Update {
        provider_code: "provider-a".into(),
        body,
    }
    .project_document_input(&op)
    .unwrap();
    assert_eq!(value["path"], json!({"provider_code":"provider-a"}));
    assert_eq!(value["body"], expected);
    assert_eq!(
        value["body"]["model_routing_policies"][0]["distribution_rule_config"]["weight"],
        json!({"type":"integer","value":80})
    );
}
