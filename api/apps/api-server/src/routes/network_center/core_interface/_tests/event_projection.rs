use super::*;
fn operation(id: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: "/api/console/settings/network-center/providers".into(),
    }
}
#[test]
fn encrypted_provider_configuration_is_absent_from_event_value_and_schema() {
    let op = operation("network_egress_providers.create");
    let input = NetworkCenterInput::CreateProvider(CreateNetworkEgressProviderBody {
        installation_id: "installation-a".into(),
        display_name: "Corporate proxy".into(),
        description: "Provider configuration".into(),
        config: json!({"password":"PROVIDER-SECRET-SENTINEL","nested":{"authorization":"BEARER-SENTINEL"}}),
    });
    let value = input.project_document_input(&op).unwrap();
    assert_eq!(
        value,
        json!({"body":{"installation_id":"installation-a","display_name":"Corporate proxy","description":"Provider configuration"}})
    );
    assert!(!value.to_string().contains("SENTINEL"));
    assert!(!NetworkCenterInput::document_input_schema(&op)
        .unwrap()
        .to_string()
        .contains("\"config\""));
    let NetworkCenterInput::CreateProvider(body) = &input else {
        unreachable!()
    };
    assert_eq!(body.config["password"], "PROVIDER-SECRET-SENTINEL");
}
#[test]
fn ordinary_route_values_keep_all_members_and_only_real_locations() {
    let op = operation("network_egress_routes.update");
    let ids = (0..90).map(|i| format!("member-{i}")).collect::<Vec<_>>();
    let value = NetworkCenterInput::UpdateRoute {
        route_id: "route-a".into(),
        body: UpdateNetworkEgressRouteBody {
            pool_member_ids: ids.clone(),
            enabled: true,
        },
    }
    .project_document_input(&op)
    .unwrap();
    assert_eq!(
        value,
        json!({"path":{"route_id":"route-a"},"body":{"pool_member_ids":ids,"enabled":true}})
    );
    assert_eq!(
        NetworkCenterInput::ListRoutes
            .project_document_input(&op)
            .unwrap(),
        json!({})
    );
    assert_eq!(
        NetworkCenterInput::SyncProvider {
            id: "provider-a".into()
        }
        .project_document_input(&op)
        .unwrap(),
        json!({"path":{"id":"provider-a"}})
    );
}
#[test]
fn provider_type_business_json_is_preserved_without_secret_name_heuristics() {
    let op = operation("network_egress_proxy_types.list");
    let schema = json!({"fields":(0..80).map(|i|json!({"key":format!("field-{i}"),"label":"Field".repeat(100)})).collect::<Vec<_>>(),"business_json":{"secret_label":"Public instruction","anything":[1,2,3]}});
    let output = NetworkCenterOutput::ProviderTypes(vec![NetworkEgressProviderTypeResponse {
        installation_id: Some("installation-a".into()),
        provider_code: "provider-a".into(),
        display_name: "Provider".into(),
        form_schema: schema.clone(),
    }]);
    let value = output.project_document_output(&op).unwrap();
    assert_eq!(value[0]["form_schema"], schema);
    assert_eq!(
        value[0]["form_schema"]["fields"].as_array().unwrap().len(),
        80
    );
    assert_eq!(
        NetworkCenterOutput::Deleted
            .project_document_output(&op)
            .unwrap(),
        Value::Null
    );
}
