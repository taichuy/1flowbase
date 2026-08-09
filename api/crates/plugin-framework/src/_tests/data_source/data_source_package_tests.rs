use std::{
    fs,
    path::{Path, PathBuf},
};

use plugin_framework::data_source_package::DataSourcePackage;
use uuid::Uuid;

struct TempDataSourcePackage {
    root: PathBuf,
}

impl TempDataSourcePackage {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("data-source-package-tests-{}", Uuid::now_v7()));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative_path: &str, content: &str) {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }
}

impl Drop for TempDataSourcePackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn loads_data_source_package_with_runtime_extension_contract() {
    let fixture = TempDataSourcePackage::new();
    fixture.write(
        "manifest.yaml",
        r#"manifest_version: 1
plugin_id: acme_hubspot_source
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Acme HubSpot Source
description: test data source package
source_kind: uploaded
trust_level: unverified
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - data_source
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.data_source/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/acme_hubspot_source
node_contributions: []
"#,
    );
    fixture.write(
        "datasource/acme_hubspot_source.yaml",
        r#"source_code: acme_hubspot_source
display_name: Acme HubSpot Source
auth_modes:
  - oauth2
capabilities:
  - native_sql/v1
  - validate_config
  - test_connection
  - discover_catalog
  - describe_resource
  - preview_read
  - import_snapshot
supports_sync: true
supports_webhook: false
resource_kinds:
  - object
config_schema:
  - key: client_id
    label: Client ID
    type: string
    required: true
"#,
    );

    let package = DataSourcePackage::load_from_dir(fixture.path()).unwrap();
    assert_eq!(package.identifier(), "acme_hubspot_source@0.1.0");
    assert_eq!(package.definition.source_code, "acme_hubspot_source");
    assert!(package.supports_native_sql());
}

fn checked_in_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/templates/data_source_http_fixture")
}

fn copy_checked_in_fixture_definition(fixture: &TempDataSourcePackage, definition: &str) {
    let source = checked_in_fixture_root();
    fixture.write(
        "manifest.yaml",
        &fs::read_to_string(source.join("manifest.yaml")).unwrap(),
    );
    fixture.write("datasource/data_source_http_fixture.yaml", definition);
}

#[test]
fn ac_004_loads_checked_in_canonical_data_model_template_at_package_intake() {
    let package = DataSourcePackage::load_from_dir(checked_in_fixture_root()).unwrap();
    let templates = &package.definition.data_model_templates;

    assert_eq!(templates.len(), 1);
    assert_eq!(
        templates[0].identity.canonical_name(),
        "data_source_http_fixture/contact_archive/v1"
    );
    assert_eq!(templates[0].operations[0].code, "archive_contact");
    assert_eq!(
        templates[0].operations[0].handler_ref.provider,
        package.definition.source_code
    );
}

#[test]
fn ac_004_rejects_duplicate_data_model_template_identity_at_package_intake() {
    let fixture = TempDataSourcePackage::new();
    let definition = fs::read_to_string(
        checked_in_fixture_root().join("datasource/data_source_http_fixture.yaml"),
    )
    .unwrap();
    let (prefix, template) = definition.split_once("data_model_templates:\n").unwrap();
    let duplicate = format!("{prefix}data_model_templates:\n{template}{template}");
    copy_checked_in_fixture_definition(&fixture, &duplicate);

    let error = DataSourcePackage::load_from_dir(fixture.path()).unwrap_err();
    assert!(error
        .to_string()
        .contains("duplicate data_model_templates identity"));
}

#[test]
fn ac_004_rejects_invalid_canonical_descriptor_at_package_intake() {
    let fixture = TempDataSourcePackage::new();
    let definition = fs::read_to_string(
        checked_in_fixture_root().join("datasource/data_source_http_fixture.yaml"),
    )
    .unwrap()
    .replace("summary: Archived contact data model", "summary: ''");
    copy_checked_in_fixture_definition(&fixture, &definition);

    let error = DataSourcePackage::load_from_dir(fixture.path()).unwrap_err();
    assert!(error
        .to_string()
        .contains("invalid data_model_templates descriptor"));
}

#[test]
fn ac_004_rejects_operation_handler_outside_provider_namespace() {
    let fixture = TempDataSourcePackage::new();
    let definition = fs::read_to_string(
        checked_in_fixture_root().join("datasource/data_source_http_fixture.yaml"),
    )
    .unwrap()
    .replace(
        "          provider: data_source_http_fixture\n          code: archive_contact",
        "          provider: foreign_provider\n          code: archive_contact",
    );
    copy_checked_in_fixture_definition(&fixture, &definition);

    let error = DataSourcePackage::load_from_dir(fixture.path()).unwrap_err();
    assert!(error
        .to_string()
        .contains("must belong to data source namespace"));
}
