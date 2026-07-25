use access_control::{
    core_settings_feature_registrations, ConsoleAuthorization, ConsoleOperationRegistry,
    ConsolePolicyGroup, SettingsFeatureRegistry,
};
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
      description_key: file-security.settings.description
      order: 100
    api_routes:
      - method: PUT
        path: /api/console/settings/file-security
console_locale_catalog:
  texts:
    - reference: file-security.settings.label
      en_us: File security
      zh_hans: 文件安全
    - reference: file-security.settings.description
      en_us: Manage file security settings
      zh_hans: 管理文件安全设置
  policy_groups: []
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
fn ac_001_host_extension_console_contribution_compiles_with_core_registry() {
    let manifest = parse_host_extension_contribution_manifest(&host_extension_console_manifest())
        .expect("valid HostExtension console contribution should parse");
    let contribution = manifest
        .console_contribution()
        .expect("valid HostExtension console contribution should convert");

    let settings_features = SettingsFeatureRegistry::compile(core_settings_feature_registrations())
        .expect("Core SettingsFeature registrations should compile");
    let registry = ConsoleOperationRegistry::compile(
        &settings_features,
        contribution.operations,
        contribution.resources,
    )
    .expect("HostExtension and Core should compile through one shared registry");

    let access = registry
        .access_for_console_route(
            "POST",
            "/api/console/file-security/files/00000000-0000-0000-0000-000000000001/scan",
        )
        .expect("HostExtension route must be present in the shared inventory");
    assert_eq!(access.operation_id, "file-security.scan");
    assert_eq!(
        access.policy_group,
        &ConsolePolicyGroup::Other("file-security.security".to_string())
    );
    assert_eq!(
        access.authorization,
        &ConsoleAuthorization::ResourceAction {
            resource_code: "file-security.secured-files".to_string(),
            action_code: "scan".to_string(),
        }
    );
    assert_eq!(
        access
            .resource_access
            .expect("resource action must resolve its resource")
            .resource_code,
        "file-security.secured-files"
    );
}

#[test]
fn rejects_host_extension_console_operation_outside_extension_namespace() {
    let raw = host_extension_console_manifest().replace(
        "operation_id: file-security.scan",
        "operation_id: other.scan",
    );

    let error = parse_host_extension_contribution_manifest(&raw).unwrap_err();

    assert!(error
        .to_string()
        .contains("console_operations[].operation_id"));
}

#[test]
fn host_extension_auth_provider_contributes_default_block_and_public_routes() {
    // Issue #1444 AC-001: a backend-only HostExtension must carry everything
    // the generic Auth host needs without shipping new Core frontend code.
    let raw = host_extension_manifest_with(
        r#"
auth_providers:
  - auth_type: file-security.qr
    display_name: QR authentication
    config_schema:
      - key: issuer
        label: Issuer
        type: string
    default_public_ui_block: |
      export default { main } satisfies BlockModule;
    public_variable_keys:
      - issuer
    public_route_ids:
      - file-security.qr.start
routes:
  - route_id: file-security.qr.start
    method: POST
    path: /api/public/auth/file-security/qr/start
    action:
      resource: file-security.qr
      action: start
workers: []
migrations: []
"#,
    );

    let manifest = parse_host_extension_contribution_manifest(&raw)
        .expect("auth provider contribution should parse");

    assert_eq!(manifest.auth_providers.len(), 1);
    assert_eq!(manifest.auth_providers[0].auth_type, "file-security.qr");
    assert!(manifest.auth_providers[0]
        .default_public_ui_block
        .contains("satisfies BlockModule"));
    assert_eq!(
        manifest.auth_providers[0].public_variable_keys,
        vec!["issuer"]
    );
    assert_eq!(
        manifest.auth_providers[0].public_route_ids,
        vec!["file-security.qr.start"]
    );

    let secret_public_variable = raw.replacen("type: string", "type: secret", 1);
    let error = parse_host_extension_contribution_manifest(&secret_public_variable).unwrap_err();
    assert!(error
        .to_string()
        .contains("public_variable_keys[] cannot reference a secret field"));

    let reserved_key = raw.replacen("key: issuer", "key: title", 1);
    let error = parse_host_extension_contribution_manifest(&reserved_key).unwrap_err();
    assert!(error
        .to_string()
        .contains("conflicts with a core authenticator field"));
}

#[test]
fn rejects_host_extension_console_owner_and_version_mismatch() {
    let owner_mismatch = host_extension_console_manifest()
        .replace("owner_id: file-security", "owner_id: other-extension");
    let error = parse_host_extension_contribution_manifest(&owner_mismatch).unwrap_err();
    assert!(error
        .to_string()
        .contains("console_operations[].owner.owner_id"));

    let version_mismatch =
        host_extension_console_manifest().replacen("version: 0.1.0", "version: 0.2.0", 1);
    let error = parse_host_extension_contribution_manifest(&version_mismatch).unwrap_err();
    assert!(error
        .to_string()
        .contains("console_operations[].owner.version"));
}

#[test]
fn rejects_invalid_host_extension_console_lifecycle_and_route() {
    let inactive = host_extension_console_manifest().replace(
        "    lifecycle: active\n    policy_group: !other file-security.security",
        "    lifecycle: inactive\n    policy_group: !other file-security.security",
    );
    let error = parse_host_extension_contribution_manifest(&inactive).unwrap_err();
    assert!(error.to_string().contains("console_operations[].lifecycle"));

    let invalid_route = host_extension_console_manifest().replace(
        "/api/console/file-security/files/{file_id}/scan",
        "/api/system/file-security/files/{file_id}/scan",
    );
    let error = parse_host_extension_contribution_manifest(&invalid_route).unwrap_err();
    assert!(error.to_string().contains("console_operations[].routes"));
}

#[test]
fn rejects_unknown_host_extension_console_group_resource_action_and_i18n_ref() {
    let unknown_group = host_extension_console_manifest().replace(
        "    policy_group: !other file-security.security",
        "    policy_group: !settings_feature missing.feature",
    );
    let error = parse_host_extension_contribution_manifest(&unknown_group).unwrap_err();
    assert!(error
        .to_string()
        .contains("console_operations[].policy_group"));

    let unknown_resource = host_extension_console_manifest().replacen(
        "      resource_code: file-security.secured-files",
        "      resource_code: missing.resource",
        1,
    );
    let error = parse_host_extension_contribution_manifest(&unknown_resource).unwrap_err();
    assert!(error
        .to_string()
        .contains("console_operations[].authorization"));

    let unknown_action = host_extension_console_manifest().replacen(
        "      action_code: scan",
        "      action_code: missing",
        1,
    );
    let error = parse_host_extension_contribution_manifest(&unknown_action).unwrap_err();
    assert!(error
        .to_string()
        .contains("console_operations[].authorization"));

    let unknown_i18n = host_extension_console_manifest().replace(
        "    - reference: file-security.console.operations.scan.label",
        "    - reference: other.console.operations.scan.label",
    );
    let error = parse_host_extension_contribution_manifest(&unknown_i18n).unwrap_err();
    assert!(error.to_string().contains("console_locale_catalog"));
}

#[test]
fn rejects_duplicate_host_extension_console_operations_resources_actions_routes_and_i18n() {
    let duplicate_operation = host_extension_console_manifest().replace(
        "console_resources:",
        r#"console_operations:
  - operation_id: file-security.scan
    owner:
      kind: host_extension
      owner_id: file-security
      version: 0.1.0
    lifecycle: active
    policy_group: !other file-security.security
    label_ref: file-security.console.operations.scan.label
    description_ref: file-security.console.operations.scan.description
    order: 100
    routes:
      - method: POST
        path: /api/console/file-security/files/{file_id}/scan
    authorization:
      kind: resource_action
      resource_code: file-security.secured-files
      action_code: scan
console_resources:"#,
    );
    let error = parse_host_extension_contribution_manifest(&duplicate_operation).unwrap_err();
    assert!(error.to_string().contains("console_operations"));

    let duplicate_resource = host_extension_console_manifest().replace(
        "console_resources:\n",
        r#"console_resources:
  - resource_code: file-security.secured-files
    owner:
      kind: host_extension
      owner_id: file-security
      version: 0.1.0
    lifecycle: active
    scope_kind: workspace
    identity_field: id
    scope_field: scope_id
    owner_field: created_by
    label_ref: file-security.console.resources.secured-files.label
    description_ref: file-security.console.resources.secured-files.description
    actions:
      - action_code: scan
        label_ref: file-security.console.resources.secured-files.actions.scan.label
        description_ref: file-security.console.resources.secured-files.actions.scan.description
console_resources:
  - resource_code: file-security.secured-files
    owner:
      kind: host_extension
      owner_id: file-security
      version: 0.1.0
    lifecycle: active
    scope_kind: workspace
    identity_field: id
    scope_field: scope_id
    owner_field: created_by
    label_ref: file-security.console.resources.secured-files.label
    description_ref: file-security.console.resources.secured-files.description
    actions:
      - action_code: scan
        label_ref: file-security.console.resources.secured-files.actions.scan.label
        description_ref: file-security.console.resources.secured-files.actions.scan.description
"#,
    );
    let error = parse_host_extension_contribution_manifest(&duplicate_resource).unwrap_err();
    assert!(error.to_string().contains("console_resources"));

    let duplicate_action = host_extension_console_manifest().replace(
        "        description_ref: file-security.console.resources.secured-files.actions.scan.description\nroutes: []",
        "        description_ref: file-security.console.resources.secured-files.actions.scan.description\n      - action_code: scan\n        label_ref: file-security.console.resources.secured-files.actions.scan.label\nroutes: []",
    );
    let error = parse_host_extension_contribution_manifest(&duplicate_action).unwrap_err();
    assert!(error.to_string().contains("console_resources"));

    let duplicate_route = host_extension_console_manifest().replace(
        "    routes:\n      - method: POST\n        path: /api/console/file-security/files/{file_id}/scan",
        "    routes:\n      - method: POST\n        path: /api/console/file-security/files/{file_id}/scan\n      - method: POST\n        path: /api/console/file-security/files/{file_id}/scan",
    );
    let error = parse_host_extension_contribution_manifest(&duplicate_route).unwrap_err();
    assert!(error.to_string().contains("console_operations[].routes"));

    let duplicate_i18n = host_extension_console_manifest().replace(
        "    - reference: file-security.console.operations.scan.label\n      en_us: Scan records\n      zh_hans: 扫描记录\n",
        "    - reference: file-security.console.operations.scan.label\n      en_us: Scan records\n      zh_hans: 扫描记录\n    - reference: file-security.console.operations.scan.label\n      en_us: Scan records\n      zh_hans: 扫描记录\n",
    );
    let error = parse_host_extension_contribution_manifest(&duplicate_i18n).unwrap_err();
    assert!(error.to_string().contains("console_locale_catalog"));
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

fn host_extension_console_manifest() -> String {
    host_extension_manifest_with(
        r#"
console_locale_catalog:
  texts:
    - reference: file-security.console.operations.scan.label
      en_us: Scan records
      zh_hans: 扫描记录
    - reference: file-security.console.operations.scan.description
      en_us: Scan a secured file
      zh_hans: 扫描受保护文件
    - reference: file-security.console.resources.secured-files.label
      en_us: Secured files
      zh_hans: 受保护文件
    - reference: file-security.console.resources.secured-files.description
      en_us: Files protected by this HostExtension
      zh_hans: 由此 HostExtension 保护的文件
    - reference: file-security.console.resources.secured-files.actions.scan.label
      en_us: Scan secured file
      zh_hans: 扫描受保护文件
    - reference: file-security.console.resources.secured-files.actions.scan.description
      en_us: Start a secured file scan
      zh_hans: 启动受保护文件扫描
    - reference: file-security.console.policy-groups.security.label
      en_us: File security
      zh_hans: 文件安全
    - reference: file-security.console.policy-groups.security.description
      en_us: Manage file security operations
      zh_hans: 管理文件安全操作
  policy_groups:
    - group_id: file-security.security
      label_ref: file-security.console.policy-groups.security.label
      description_ref: file-security.console.policy-groups.security.description
console_operations:
  - operation_id: file-security.scan
    owner:
      kind: host_extension
      owner_id: file-security
      version: 0.1.0
    lifecycle: active
    policy_group: !other file-security.security
    label_ref: file-security.console.operations.scan.label
    description_ref: file-security.console.operations.scan.description
    order: 100
    routes:
      - method: POST
        path: /api/console/file-security/files/{file_id}/scan
    authorization:
      kind: resource_action
      resource_code: file-security.secured-files
      action_code: scan
console_resources:
  - resource_code: file-security.secured-files
    owner:
      kind: host_extension
      owner_id: file-security
      version: 0.1.0
    lifecycle: active
    scope_kind: workspace
    identity_field: id
    scope_field: scope_id
    owner_field: created_by
    label_ref: file-security.console.resources.secured-files.label
    description_ref: file-security.console.resources.secured-files.description
    actions:
      - action_code: scan
        label_ref: file-security.console.resources.secured-files.actions.scan.label
        description_ref: file-security.console.resources.secured-files.actions.scan.description
routes: []
workers: []
migrations: []
"#,
    )
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
