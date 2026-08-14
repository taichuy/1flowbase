use plugin_framework::extension_bus::parse_deployment_plugin_set;

#[test]
fn deployment_plugin_set_parses_typed_inventory() {
    let set = parse_deployment_plugin_set(
        r#"schema_version: 1flowbase.plugin-set/v1
set_id: default
host_extensions:
  - official.identity-host
runtime_extensions: []
capability_plugins:
  - 1flowbase
"#,
    )
    .unwrap();

    assert_eq!(set.set_id(), "default");
    assert_eq!(set.host_extension_ids(), ["official.identity-host"]);
    assert!(set.runtime_extension_ids().is_empty());
    assert_eq!(set.capability_plugin_ids(), ["1flowbase"]);
}

#[test]
fn deployment_plugin_set_rejects_duplicate_set_id_field() {
    let error = parse_deployment_plugin_set(
        r#"schema_version: 1flowbase.plugin-set/v1
set_id: first
set_id: second
host_extensions: []
runtime_extensions: []
capability_plugins: []
"#,
    )
    .unwrap_err();

    assert!(error.to_string().contains("duplicate field `set_id`"));
}

#[test]
fn deployment_plugin_set_rejects_unknown_field() {
    let error = parse_deployment_plugin_set(
        r#"schema_version: 1flowbase.plugin-set/v1
set_id: default
host_extensions: []
runtime_extensions: []
capability_plugins: []
fallback_plugins: []
"#,
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("unknown field `fallback_plugins`"));
}

#[test]
fn deployment_plugin_set_rejects_wrong_schema_and_empty_identifier() {
    let wrong_schema = parse_deployment_plugin_set(
        r#"schema_version: 1flowbase.plugin-set/v2
set_id: default
host_extensions: []
runtime_extensions: []
capability_plugins: []
"#,
    )
    .unwrap_err();
    assert!(wrong_schema
        .to_string()
        .contains("schema_version must be 1flowbase.plugin-set/v1"));

    let empty_id = parse_deployment_plugin_set(
        r#"schema_version: 1flowbase.plugin-set/v1
set_id: ""
host_extensions: []
runtime_extensions: []
capability_plugins: []
"#,
    )
    .unwrap_err();
    assert!(empty_id
        .to_string()
        .contains("set_id contains invalid identifier"));
}

#[test]
fn deployment_plugin_set_rejects_duplicate_module_id_across_categories() {
    let error = parse_deployment_plugin_set(
        r#"schema_version: 1flowbase.plugin-set/v1
set_id: default
host_extensions:
  - duplicate.module
runtime_extensions: []
capability_plugins:
  - duplicate.module
"#,
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("module id duplicate.module is declared more than once"));
}
