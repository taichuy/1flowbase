use super::*;
fn operation(id: &str) -> ApiOperationContext {
    ApiOperationContext {
        interface_id: id.into(),
        method: "GET".into(),
        path: "/api/console/settings/network-center/proxy-plugins/official-catalog".into(),
    }
}
fn locale() -> ConsoleLocaleHints {
    ConsoleLocaleHints::from_headers(&axum::http::HeaderMap::new())
}
#[test]
fn documented_query_locale_is_preserved_and_absent_optional_query_fields_are_omitted() {
    let op = operation("network_egress_plugins.official_catalog.view");
    let query = OfficialPluginCatalogQuery {
        plugin_type: None,
        locale: Some("zh_Hans".into()),
        q: Some("代理".into()),
        cursor: None,
        limit: Some(25),
    };
    assert_eq!(
        NetworkPluginInput::ListOfficial {
            query,
            locale: locale()
        }
        .project_document_input(&op)
        .unwrap(),
        json!({"query":{"locale":"zh_Hans","q":"代理","limit":25}})
    );
    assert_eq!(
        NetworkPluginInput::ListFamilies { locale: locale() }
            .project_document_input(&op)
            .unwrap(),
        json!({})
    );
}
#[test]
fn package_upload_only_blocks_request_and_keeps_family_json_available() {
    let op = operation("network_egress_plugins.install.upload");
    assert_eq!(
        NetworkPluginInput::document_input_unavailable(&op),
        Some(domain::event_center::ApiEventUnavailableReason::BinaryRequest)
    );
    assert_eq!(NetworkPluginOutput::document_output_unavailable(&op), None);
    assert!(NetworkPluginInput::InstallUploaded {
        file_name: "private.pkg".into(),
        package_bytes: b"PACKAGE-SECRET-SENTINEL".to_vec()
    }
    .project_document_input(&op)
    .is_err());
    assert_eq!(
        NetworkPluginInput::document_input_unavailable(&operation(
            "network_egress_plugins.install.official"
        )),
        None
    );
}
#[test]
fn family_outputs_keep_every_real_version_and_long_names() {
    let op = operation("network_egress_plugins.families.view");
    let families = vec![NetworkEgressPluginFamilyResponse {
        provider_code: "provider-a".into(),
        display_name: "代理插件".repeat(300),
        current_installation_id: "installation-a".into(),
        current_version: "1.0.0".into(),
        can_uninstall: true,
        installed_versions: (0..90)
            .map(|i| NetworkEgressPluginInstalledVersionResponse {
                installation_id: format!("installation-{i}"),
                plugin_version: format!("1.0.{i}"),
                is_current: i == 0,
                can_uninstall: i != 0,
            })
            .collect(),
    }];
    let expected = serde_json::to_value(&families).unwrap();
    assert_eq!(
        NetworkPluginOutput::Families(families)
            .project_document_output(&op)
            .unwrap(),
        expected
    );
    assert_eq!(
        NetworkPluginOutput::Empty
            .project_document_output(&op)
            .unwrap(),
        Value::Null
    );
}
#[test]
fn family_and_version_uninstall_have_distinct_actual_path_parameters() {
    let op = operation("network_egress_plugins.families.uninstall");
    assert_eq!(
        NetworkPluginInput::UninstallFamily {
            provider_code: "provider-a".into(),
            locale: locale()
        }
        .project_document_input(&op)
        .unwrap(),
        json!({"path":{"provider_code":"provider-a"}})
    );
    assert_eq!(
        NetworkPluginInput::UninstallVersion {
            provider_code: "provider-a".into(),
            installation_id: "installation-a".into(),
            locale: locale()
        }
        .project_document_input(&op)
        .unwrap(),
        json!({"path":{"provider_code":"provider-a","installation_id":"installation-a"}})
    );
}
