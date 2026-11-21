use super::{parse, record_fixture};
use serde_json::{Value, json};

fn provider() -> Value {
    let mut value = record_fixture();
    value["permissions"]["secrets"] = json!("host_managed");
    value["resource_exports"] = json!([{
        "resource_code":"hosts-v1", "collections":[{"collection_code":"hosts","access":"read_only"}],
        "record_resources":[{"resource_code":"hosts","collection_code":"hosts","id_field":"host_id"}],
        "credentials":"read_only"
    }]);
    value
}
fn consumer() -> Value {
    let mut value = record_fixture();
    value["data_models"] = json!([]);
    value["managed"]["module"]["contributions"]
        .as_array_mut()
        .unwrap()
        .pop();
    value["managed"]["execution_bindings"]
        .as_array_mut()
        .unwrap()
        .pop();
    value["permissions"]["secrets"] = json!("host_managed");
    value["plugin_dependencies"] = json!([{"publisher_namespace":"provider","plugin_code":"ssh"}]);
    value["resource_imports"] = json!([{
        "provider":{"publisher_namespace":"provider","plugin_code":"ssh"},"resource_code":"hosts-v1",
        "collections":[{"collection_code":"hosts","access":"read_only"}],"record_resources":["hosts"],"credentials":"read_only"
    }]);
    value
}

#[test]
fn resource_provider_is_standalone_and_importer_has_no_duplicate_schema() {
    let parsed = parse(&provider()).unwrap();
    assert!(parsed.plugin_dependencies.is_empty());
    assert_eq!(parsed.resource_exports[0].resource_code, "hosts-v1");
    let parsed = parse(&consumer()).unwrap();
    assert!(parsed.data_models.is_empty());
    assert_eq!(parsed.resource_imports[0].provider.plugin_code, "ssh");
}

#[test]
fn resource_declarations_reject_undeclared_prerequisites_and_owned_schema_copies() {
    let mut value = consumer();
    value["plugin_dependencies"] = json!([]);
    assert!(
        parse(&value)
            .unwrap_err()
            .to_string()
            .contains("dependency")
    );
    let mut value = consumer();
    value["data_models"] = record_fixture()["data_models"].clone();
    assert!(
        parse(&value)
            .unwrap_err()
            .to_string()
            .contains("duplicate owned schema")
    );
}

#[test]
fn resource_exports_cannot_invent_ownership_or_drop_protected_bindings() {
    let mut value = provider();
    value["resource_exports"][0]["collections"][0]["collection_code"] = json!("other");
    assert!(parse(&value).is_err());
    let mut value = provider();
    value["resource_exports"][0]["record_resources"] = json!([]);
    assert!(parse(&value).unwrap_err().to_string().contains("protected"));
    let mut value = provider();
    value["resource_exports"][0]["record_resources"][0]["id_field"] = json!("other");
    assert!(parse(&value).is_err());
}

#[test]
fn resource_access_is_closed_and_credential_import_requires_host_managed_permission() {
    let mut value = consumer();
    value["resource_imports"][0]["collections"][0]["access"] = json!("all");
    assert!(parse(&value).is_err());
    let mut value = consumer();
    value["permissions"]["secrets"] = json!("none");
    assert!(parse(&value).is_err());
    use extension_contracts::PluginResourceAccess::{ReadOnly, ReadWrite};
    assert!(ReadWrite.allows(ReadOnly));
    assert!(!ReadOnly.allows(ReadWrite));
}
