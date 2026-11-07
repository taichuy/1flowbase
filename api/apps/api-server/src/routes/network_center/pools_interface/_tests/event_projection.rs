use super::*;
fn operation(id: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: "/api/console/network-center/pools/:pool_id/members/static-http".into(),
    }
}
#[test]
fn static_proxy_password_is_omitted_while_public_connection_fields_are_kept() {
    let op = operation("network_egress_pool_members.create_static_http");
    let input = NetworkPoolsInput::AddStatic {
        pool_id: "pool-a".into(),
        body: AddStaticHttpProxyToPoolBody {
            display_name: "Proxy".into(),
            host: "proxy.example.test".into(),
            port: 3128,
            username: "alice".into(),
            password: "STATIC-PASSWORD-SENTINEL".into(),
            enabled: true,
            sequence: 4,
        },
    };
    let value = input.project_document_input(&op).unwrap();
    assert_eq!(
        value,
        json!({"path":{"pool_id":"pool-a"},"body":{"display_name":"Proxy","host":"proxy.example.test","port":3128,"username":"alice","enabled":true,"sequence":4}})
    );
    assert!(!value.to_string().contains("SENTINEL"));
    let schema = NetworkPoolsInput::document_input_schema(&op)
        .unwrap()
        .to_string();
    assert!(!schema.contains("password"));
    assert!(schema.contains("username"));
    let NetworkPoolsInput::AddStatic { body, .. } = &input else {
        unreachable!()
    };
    assert_eq!(body.password, "STATIC-PASSWORD-SENTINEL");
}
#[test]
fn proxy_config_projection_uses_the_existing_credential_owner_public_field_boundary() {
    let op = operation("network_egress_proxies.create");
    for provider_code in ["builtin_static_http", "builtin_static_socks5"] {
        let input = NetworkPoolsInput::CreateProxy(CreateNetworkEgressProxyBody {
            provider_code: provider_code.into(),
            display_name: "Proxy".into(),
            description: "Corporate".into(),
            config: json!({"host":"proxy.example.test","port":3128,"username":"alice","password":"PASSWORD-SENTINEL","unknown_private_field":"PRIVATE-SENTINEL"}),
        });
        let value = input.project_document_input(&op).unwrap();
        assert_eq!(
            value["body"]["config"],
            json!({"host":"proxy.example.test","port":3128,"username":"alice"})
        );
        assert!(!value.to_string().contains("SENTINEL"));
    }
    let extension = NetworkPoolsInput::CreateProxy(CreateNetworkEgressProxyBody {
        provider_code: "extension-proxy".into(),
        display_name: "Managed proxy".into(),
        description: "Remote".into(),
        config: json!({"host":"SECRET-URL-SENTINEL","token":"EXTENSION-SECRET-SENTINEL"}),
    });
    let value = extension.project_document_input(&op).unwrap();
    assert_eq!(value["body"]["config"], json!({}));
    assert_eq!(value["body"]["display_name"], "Managed proxy");
    assert!(!NetworkPoolsInput::document_input_schema(&op)
        .unwrap()
        .to_string()
        .contains("password"));
}
#[test]
fn already_public_proxy_response_keeps_all_business_configuration_and_form_metadata() {
    let op = operation("network_egress_proxies.get");
    let response = NetworkEgressProxyResponse {
        provider_id: "provider-a".into(),
        provider_code: "builtin_static_http".into(),
        display_name: "Proxy".into(),
        description: "Public config".into(),
        config: json!({"host":"proxy.example.test","port":"3128","username":"alice"}),
        configured_secret_fields: vec!["password".into()],
        form_schema: json!({"fields":[{"key":"password","type":"secret","label":"Public password label"}],"business_metadata":{"secret_label":"Public instruction","values":(0..80).collect::<Vec<_>>()}}),
    };
    let expected = serde_json::to_value(&response).unwrap();
    assert_eq!(
        NetworkPoolsOutput::Proxy(response)
            .project_document_output(&op)
            .unwrap(),
        expected
    );
}
#[test]
fn batch_member_selection_keeps_complete_wire_enum_and_path_shape() {
    let op = operation("network_egress_pool_members.batch_delete");
    let ids = (0..80).map(|i| format!("member-{i}")).collect::<Vec<_>>();
    let selected = NetworkPoolsInput::DeleteMembers {
        pool_id: "pool-a".into(),
        body: DeleteNetworkEgressPoolMembersBody::Selected {
            member_ids: ids.clone(),
        },
    }
    .project_document_input(&op)
    .unwrap();
    assert_eq!(
        selected,
        json!({"path":{"pool_id":"pool-a"},"body":{"selection":"selected","member_ids":ids}})
    );
    let all = NetworkPoolsInput::DeleteMembers {
        pool_id: "pool-a".into(),
        body: DeleteNetworkEgressPoolMembersBody::All,
    }
    .project_document_input(&op)
    .unwrap();
    assert_eq!(
        all,
        json!({"path":{"pool_id":"pool-a"},"body":{"selection":"all"}})
    );
}
