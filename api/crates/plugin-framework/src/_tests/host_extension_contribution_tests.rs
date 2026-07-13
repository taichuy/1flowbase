use plugin_framework::parse_host_extension_contribution_manifest;

fn assert_shared_settings_feature_contract(
    registrations: &[access_control::SettingsFeatureRegistration],
) {
    assert!(!registrations.is_empty());
}

#[test]
fn ac_001_host_extension_uses_shared_settings_feature_registration_contract() {
    let raw = host_extension_manifest_with(
        r#"
settings_features:
  - feature_id: file-security.settings
    owner:
      kind: host_extension
      owner_id: file-security
      version: 0.1.0
    lifecycle: active
    console_surface:
      route_id: file-security.settings
      surface_key: file-security.settings
      path: /settings/file-security
      label_key: file-security.settings.label
      order: 100
    api_routes:
      - method: PUT
        path: /api/console/settings/file-security
routes: []
workers: []
migrations: []
"#,
    );

    let manifest = parse_host_extension_contribution_manifest(&raw).unwrap();

    assert_shared_settings_feature_contract(&manifest.settings_features);
    assert_eq!(
        manifest.settings_features[0].feature_id,
        "file-security.settings"
    );
}

#[test]
fn parses_pre_state_infrastructure_provider_manifest() {
    let raw = r#"
schema_version: 1flowbase.host-extension/v1
extension_id: redis-infra-host
version: 0.1.0
bootstrap_phase: pre_state
native:
  abi_version: 1flowbase.host.native/v1
  library: lib/redis_infra_host
  entry_symbol: oneflowbase_host_extension_entry_v1
owned_resources: []
extends_resources: []
infrastructure_providers:
  - contract: storage-ephemeral
    provider_code: redis
    display_name: Redis
    config_ref: secret://system/redis-infra-host/config
    config_schema: []
routes: []
workers: []
migrations: []
"#;

    let manifest = parse_host_extension_contribution_manifest(raw).unwrap();

    assert_eq!(manifest.extension_id, "redis-infra-host");
    assert_eq!(manifest.bootstrap_phase.as_str(), "pre_state");
    assert_eq!(
        manifest.infrastructure_providers[0].contract,
        "storage-ephemeral"
    );
}

#[test]
fn parses_infrastructure_provider_config_schema_before_runtime_activation() {
    let raw = r#"
schema_version: 1flowbase.host-extension/v1
extension_id: redis-infra-host
version: 0.1.0
bootstrap_phase: pre_state
native:
  abi_version: 1flowbase.host.native/v1
  library: lib/redis_infra_host
  entry_symbol: oneflowbase_host_extension_entry_v1
owned_resources: []
extends_resources: []
infrastructure_providers:
  - contract: storage-ephemeral
    provider_code: redis
    display_name: Redis
    description: Redis backed host infrastructure.
    config_ref: secret://system/redis-infra-host/config
    config_schema:
      - key: host
        label: Host
        type: string
        required: true
      - key: port
        label: Port
        type: number
        required: true
        default_value: 6379
      - key: password_ref
        label: Password Secret Ref
        type: string
        send_mode: secret_ref
  - contract: cache-store
    provider_code: redis
    display_name: Redis
    description: Redis backed host infrastructure.
    config_ref: secret://system/redis-infra-host/config
    config_schema:
      - key: host
        label: Host
        type: string
        required: true
routes: []
workers: []
migrations: []
"#;

    let manifest = parse_host_extension_contribution_manifest(raw).unwrap();
    assert_eq!(manifest.infrastructure_providers.len(), 2);
    let provider = &manifest.infrastructure_providers[0];

    assert_eq!(provider.contract, "storage-ephemeral");
    assert_eq!(provider.provider_code, "redis");
    assert_eq!(provider.display_name, "Redis");
    assert_eq!(
        provider.description.as_deref(),
        Some("Redis backed host infrastructure.")
    );
    assert_eq!(provider.config_schema[0].key, "host");
    assert_eq!(
        provider.config_schema[2].send_mode.as_deref(),
        Some("secret_ref")
    );
}

#[test]
fn rejects_unknown_schema_version() {
    let raw = r#"
schema_version: wrong/v1
extension_id: redis-infra-host
version: 0.1.0
bootstrap_phase: pre_state
native:
  abi_version: 1flowbase.host.native/v1
  library: lib/redis_infra_host
  entry_symbol: oneflowbase_host_extension_entry_v1
owned_resources: []
extends_resources: []
infrastructure_providers: []
routes: []
workers: []
migrations: []
"#;

    let err = parse_host_extension_contribution_manifest(raw).unwrap_err();
    assert!(err.to_string().contains("schema_version"));
}

#[test]
fn parses_structured_route_worker_and_migration_declarations() {
    let raw = r#"
schema_version: 1flowbase.host-extension/v1
extension_id: file-security
version: 0.1.0
bootstrap_phase: boot
native:
  abi_version: 1flowbase.host.native/v1
  library: lib/file_security_host
  entry_symbol: oneflowbase_host_extension_entry_v1
owned_resources: []
extends_resources: []
infrastructure_providers: []
routes:
  - route_id: file-security.scan-report
    method: GET
    path: /api/system/file-security/files/{file_id}/scan-report
    action:
      resource: file_scan_reports
      action: get
workers:
  - worker_id: file-security.scan-worker
    queue: file-security.scan
    handler: scan_file
migrations:
  - id: 0001_create_file_security_tables
    path: migrations/postgres/0001_create_file_security_tables.sql
"#;

    let manifest = parse_host_extension_contribution_manifest(raw).unwrap();

    assert_eq!(manifest.routes[0].route_id, "file-security.scan-report");
    assert_eq!(manifest.routes[0].method, "GET");
    assert_eq!(
        manifest.routes[0].path,
        "/api/system/file-security/files/{file_id}/scan-report"
    );
    assert_eq!(manifest.routes[0].action.resource, "file_scan_reports");
    assert_eq!(manifest.routes[0].action.action, "get");
    assert_eq!(manifest.workers[0].worker_id, "file-security.scan-worker");
    assert_eq!(manifest.workers[0].queue, "file-security.scan");
    assert_eq!(manifest.workers[0].handler, "scan_file");
    assert_eq!(
        manifest.migrations[0].path,
        "migrations/postgres/0001_create_file_security_tables.sql"
    );
}

#[test]
fn existing_manifest_defaults_console_surfaces_to_empty() {
    let raw = host_extension_manifest_with(
        r#"
routes: []
workers: []
migrations: []
"#,
    );

    let manifest = parse_host_extension_contribution_manifest(&raw).unwrap();

    assert!(manifest.console_surfaces.route_definitions.is_empty());
    assert!(manifest.console_surfaces.navigation_items.is_empty());
    assert!(manifest.console_surfaces.permission_bindings.is_empty());
}

#[test]
fn parses_console_surface_route_navigation_and_permission_bindings() {
    let raw = host_extension_manifest_with(
        r#"
routes: []
console_surfaces:
  route_definitions:
    - route_id: file-security.settings
      surface_key: file-security
      path: /settings/file-security
      surface_kind: host_extension
  navigation_items:
    - item_id: file-security.settings
      route_id: file-security.settings
      parent_item_id: settings
      label_key: auto.file_security
      navigation_slot: settings
      order: 900
  permission_bindings:
    - binding_id: file-security.settings.view
      route_id: file-security.settings
      permission_codes:
        - plugin_config.view.all
      requirement: any_permission
workers: []
migrations: []
"#,
    );

    let manifest = parse_host_extension_contribution_manifest(&raw).unwrap();
    let surfaces = &manifest.console_surfaces;

    assert_eq!(
        surfaces.route_definitions[0].route_id,
        "file-security.settings"
    );
    assert_eq!(surfaces.route_definitions[0].surface_key, "file-security");
    assert_eq!(surfaces.navigation_items[0].parent_item_id, "settings");
    assert_eq!(surfaces.navigation_items[0].order, 900);
    assert_eq!(
        surfaces.permission_bindings[0].permission_codes,
        vec!["plugin_config.view.all"]
    );
}

#[test]
fn rejects_console_surface_kind_that_is_not_host_extension() {
    let raw = host_extension_manifest_with(
        r#"
routes: []
console_surfaces:
  route_definitions:
    - route_id: file-security.settings
      surface_key: file-security
      path: /settings/file-security
      surface_kind: system
  navigation_items: []
  permission_bindings: []
workers: []
migrations: []
"#,
    );

    let err = parse_host_extension_contribution_manifest(&raw).unwrap_err();
    assert!(err
        .to_string()
        .contains("console_surfaces.route_definitions[].surface_kind"));
}

#[test]
fn rejects_console_surface_path_outside_settings_prefix() {
    let raw = host_extension_manifest_with(
        r#"
routes: []
console_surfaces:
  route_definitions:
    - route_id: file-security.settings
      surface_key: file-security
      path: /workspace/file-security
      surface_kind: host_extension
  navigation_items: []
  permission_bindings: []
workers: []
migrations: []
"#,
    );

    let err = parse_host_extension_contribution_manifest(&raw).unwrap_err();
    assert!(err
        .to_string()
        .contains("console_surfaces.route_definitions[].path"));
}

#[test]
fn rejects_console_navigation_item_unknown_route_id() {
    let raw = host_extension_manifest_with(
        r#"
routes: []
console_surfaces:
  route_definitions:
    - route_id: file-security.settings
      surface_key: file-security
      path: /settings/file-security
      surface_kind: host_extension
  navigation_items:
    - item_id: file-security.settings
      route_id: file-security.missing
      parent_item_id: settings
      label_key: auto.file_security
      navigation_slot: settings
      order: 900
  permission_bindings: []
workers: []
migrations: []
"#,
    );

    let err = parse_host_extension_contribution_manifest(&raw).unwrap_err();
    assert!(err
        .to_string()
        .contains("console_surfaces.navigation_items[].route_id"));
}

#[test]
fn rejects_console_surface_ids_outside_extension_namespace() {
    let raw = host_extension_manifest_with(
        r#"
routes: []
console_surfaces:
  route_definitions:
    - route_id: other.settings
      surface_key: file-security
      path: /settings/file-security
      surface_kind: host_extension
  navigation_items: []
  permission_bindings: []
workers: []
migrations: []
"#,
    );

    let err = parse_host_extension_contribution_manifest(&raw).unwrap_err();
    assert!(err
        .to_string()
        .contains("console_surfaces.route_definitions[].route_id"));
}

#[test]
fn rejects_duplicate_console_surface_route_id_in_same_manifest() {
    let raw = host_extension_manifest_with(
        r#"
routes: []
console_surfaces:
  route_definitions:
    - route_id: file-security.settings
      surface_key: file-security
      path: /settings/file-security
      surface_kind: host_extension
    - route_id: file-security.settings
      surface_key: file-security-duplicate
      path: /settings/file-security-duplicate
      surface_kind: host_extension
  navigation_items: []
  permission_bindings: []
workers: []
migrations: []
"#,
    );

    let err = parse_host_extension_contribution_manifest(&raw).unwrap_err();
    assert!(err
        .to_string()
        .contains("console_surfaces.route_definitions[].route_id"));
}

#[test]
fn rejects_duplicate_console_surface_path_in_same_manifest() {
    let raw = host_extension_manifest_with(
        r#"
routes: []
console_surfaces:
  route_definitions:
    - route_id: file-security.settings
      surface_key: file-security
      path: /settings/file-security
      surface_kind: host_extension
    - route_id: file-security.audit
      surface_key: file-security-audit
      path: /settings/file-security
      surface_kind: host_extension
  navigation_items: []
  permission_bindings: []
workers: []
migrations: []
"#,
    );

    let err = parse_host_extension_contribution_manifest(&raw).unwrap_err();
    assert!(err
        .to_string()
        .contains("console_surfaces.route_definitions[].path"));
}

#[test]
fn rejects_duplicate_console_navigation_item_id_in_same_manifest() {
    let raw = host_extension_manifest_with(
        r#"
routes: []
console_surfaces:
  route_definitions:
    - route_id: file-security.settings
      surface_key: file-security
      path: /settings/file-security
      surface_kind: host_extension
    - route_id: file-security.audit
      surface_key: file-security-audit
      path: /settings/file-security-audit
      surface_kind: host_extension
  navigation_items:
    - item_id: file-security.settings
      route_id: file-security.settings
      parent_item_id: settings
      label_key: auto.file_security
      navigation_slot: settings
      order: 900
    - item_id: file-security.settings
      route_id: file-security.audit
      parent_item_id: settings
      label_key: auto.file_security
      navigation_slot: settings
      order: 901
  permission_bindings: []
workers: []
migrations: []
"#,
    );

    let err = parse_host_extension_contribution_manifest(&raw).unwrap_err();
    assert!(err
        .to_string()
        .contains("console_surfaces.navigation_items[].item_id"));
}

#[test]
fn rejects_duplicate_console_permission_binding_id_in_same_manifest() {
    let raw = host_extension_manifest_with(
        r#"
routes: []
console_surfaces:
  route_definitions:
    - route_id: file-security.settings
      surface_key: file-security
      path: /settings/file-security
      surface_kind: host_extension
  navigation_items: []
  permission_bindings:
    - binding_id: file-security.settings.view
      route_id: file-security.settings
      permission_codes:
        - plugin_config.view.all
      requirement: any_permission
    - binding_id: file-security.settings.view
      route_id: file-security.settings
      permission_codes:
        - plugin_config.configure.all
      requirement: any_permission
workers: []
migrations: []
"#,
    );

    let err = parse_host_extension_contribution_manifest(&raw).unwrap_err();
    assert!(err
        .to_string()
        .contains("console_surfaces.permission_bindings[].binding_id"));
}

#[test]
fn rejects_console_permission_any_permission_without_permission_codes() {
    let raw = host_extension_manifest_with(
        r#"
routes: []
console_surfaces:
  route_definitions:
    - route_id: file-security.settings
      surface_key: file-security
      path: /settings/file-security
      surface_kind: host_extension
  navigation_items: []
  permission_bindings:
    - binding_id: file-security.settings.view
      route_id: file-security.settings
      permission_codes: []
      requirement: any_permission
workers: []
migrations: []
"#,
    );

    let err = parse_host_extension_contribution_manifest(&raw).unwrap_err();
    assert!(err
        .to_string()
        .contains("console_surfaces.permission_bindings[].permission_codes"));
}

#[test]
fn rejects_console_permission_codes_for_authenticated_requirement() {
    let raw = host_extension_manifest_with(
        r#"
routes: []
console_surfaces:
  route_definitions:
    - route_id: file-security.settings
      surface_key: file-security
      path: /settings/file-security
      surface_kind: host_extension
  navigation_items: []
  permission_bindings:
    - binding_id: file-security.settings.view
      route_id: file-security.settings
      permission_codes:
        - plugin_config.view.all
      requirement: authenticated
workers: []
migrations: []
"#,
    );

    let err = parse_host_extension_contribution_manifest(&raw).unwrap_err();
    assert!(err
        .to_string()
        .contains("console_surfaces.permission_bindings[].permission_codes"));
}

#[test]
fn rejects_route_path_outside_controlled_host_prefixes() {
    let raw = host_extension_manifest_with(
        r#"
routes:
  - route_id: file-security.scan-report
    method: GET
    path: /api/raw/file-security
    action:
      resource: file_scan_reports
      action: get
workers: []
migrations: []
"#,
    );

    let err = parse_host_extension_contribution_manifest(&raw).unwrap_err();
    assert!(err.to_string().contains("routes[].path"));
}

#[test]
fn rejects_worker_id_without_extension_owned_prefix() {
    let raw = host_extension_manifest_with(
        r#"
routes: []
workers:
  - worker_id: other.scan-worker
    queue: file-security.scan
    handler: scan_file
migrations: []
"#,
    );

    let err = parse_host_extension_contribution_manifest(&raw).unwrap_err();
    assert!(err.to_string().contains("workers[].worker_id"));
}

#[test]
fn rejects_migration_path_outside_postgres_migrations() {
    let raw = host_extension_manifest_with(
        r#"
routes: []
workers: []
migrations:
  - id: 0001_create_file_security_tables
    path: ../core/0001.sql
"#,
    );

    let err = parse_host_extension_contribution_manifest(&raw).unwrap_err();
    assert!(err.to_string().contains("migrations[].path"));
}

fn host_extension_manifest_with(contributions: &str) -> String {
    format!(
        r#"
schema_version: 1flowbase.host-extension/v1
extension_id: file-security
version: 0.1.0
bootstrap_phase: boot
native:
  abi_version: 1flowbase.host.native/v1
  library: lib/file_security_host
  entry_symbol: oneflowbase_host_extension_entry_v1
owned_resources: []
extends_resources: []
infrastructure_providers: []
{contributions}
"#
    )
}
