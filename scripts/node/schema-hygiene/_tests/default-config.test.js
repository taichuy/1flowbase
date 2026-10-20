const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  collectSchemaInventory,
  evaluateSchemaHygiene,
  loadConfig,
  main,
} = require('../core.js');

const { writeFile, createRepoWithMigrations, createRepoWithMigration } = require('./support.js');

test('default schema hygiene config exempts bounded projection and release tables', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const inventory = collectSchemaInventory({ repoRoot });
  const report = evaluateSchemaHygiene({
    inventory,
    config: loadConfig(repoRoot),
  });

  for (const [tableName, expectedAction] of [
    ['plugin_package_catalog_projection', 'bounded_projection_exempt'],
    ['extension_artifact_instances', 'bounded_projection_exempt'],
    ['plugin_artifact_cleanup_jobs', 'system_runtime_cleanup_queue_declared'],
    ['lifecycle_outbox_deliveries', 'subscriber_delivery_outbox_declared'],
    ['i18n_catalog_release_files', 'bounded_release_child_declared'],
    ['i18n_catalog_release_messages', 'bounded_release_child_declared'],
    ['i18n_catalog_release_translations', 'bounded_release_child_declared'],
    ['i18n_catalog_releases', 'system_release_ledger_declared'],
    ['workspace_i18n_catalog_custom_translations', 'bounded_workspace_projection_declared'],
    ['workspace_i18n_catalog_states', 'workspace_state_root_declared'],
    ['workspace_i18n_catalog_obsolete_messages', 'bounded_workspace_projection_declared'],
    ['workspace_i18n_catalog_overrides', 'bounded_workspace_projection_declared'],
  ]) {
    const table = report.tables.find((candidate) => candidate.name === tableName);
    assert.ok(table.exemption.kind);
    assert.equal(table.findings.some((finding) => finding.rule === 'managed-table-needs-owner-review'), false);
    assert.equal(table.platformReadiness.recommendedActions.includes('needs_owner_review'), false);
    assert.deepEqual(table.platformReadiness.recommendedActions, [expectedAction]);
  }

  assert.equal(report.summary.errors, 0);
});

test('default schema hygiene config distinguishes bounded system catalog state from deferred scope migrations', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const inventory = collectSchemaInventory({ repoRoot });
  const report = evaluateSchemaHygiene({
    inventory,
    config: loadConfig(repoRoot),
  });

  for (const [tableName, expectedCategory, expectedAction] of [
    ['console_permission_catalog_sync', 'system_permission_catalog_state', 'system_permission_catalog_state_declared'],
    ['console_permission_catalog_groups', 'system_permission_catalog', 'system_permission_catalog_declared'],
    ['console_permission_catalog_operations', 'system_permission_catalog', 'system_permission_catalog_declared'],
    ['native_plugin_application_requests', 'system_native_extension_request', 'system_native_extension_request_declared'],
    ['native_plugin_targets', 'system_native_extension_target', 'system_native_extension_target_declared'],
    ['plugin_settings_template_applications', 'system_native_template_application', 'system_native_template_application_declared'],
    ['plugin_settings_template_defaults', 'system_native_template_default', 'system_native_template_default_declared'],
    ['application_run_log_tasks', 'runtime_task_projection', 'declare_generation_rule'],
  ]) {
    const table = report.tables.find((candidate) => candidate.name === tableName);
    assert.deepEqual(table.findings, [], `${tableName} should have no schema hygiene findings`);
    assert.equal(table.platformReadiness.category, expectedCategory);
    assert.deepEqual(table.platformReadiness.recommendedActions, [expectedAction]);
  }

  for (const tableName of [
    'plugin_contribution_authorization_revisions',
    'plugin_contribution_authorizations',
    'provider_protocol_capsules',
  ]) {
    const table = report.tables.find((candidate) => candidate.name === tableName);
    assert.deepEqual(table.findings, [], `${tableName} must retain its bounded owner contract`);
    assert.equal(table.platformReadiness.severity, 'ok');
    assert.equal(table.platformReadiness.recommendedActions.includes('needs_owner_review'), false);
  }

  assert.equal(report.summary.errors, 0);
});

test('default schema hygiene config declares issue 1082 root and auth reference profiles', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const inventory = collectSchemaInventory({ repoRoot });
  const report = evaluateSchemaHygiene({
    inventory,
    config: loadConfig(repoRoot),
  });

  for (const [tableName, expectedCategory] of [
    ['tenants', 'routing_root_reference'],
    ['workspaces', 'workspace_routing_root'],
    ['users', 'global_identity_reference'],
    ['authentication_connections', 'system_reference_table'],
    ['login_entries', 'system_reference_table'],
    ['user_auth_identities', 'global_identity_reference'],
  ]) {
    const table = report.tables.find((candidate) => candidate.name === tableName);
    assert.equal(
      table.findings.some((finding) => finding.rule === 'managed-table-needs-owner-review'),
      false
    );
    assert.equal(table.platformReadiness.category, expectedCategory);
    assert.equal(table.platformReadiness.severity, 'ok');
    assert.match(table.platformReadiness.reason, /(routing|identity|system|auth)/u);
  }
});

test('default schema hygiene config declares trace projection owner chain', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const inventory = collectSchemaInventory({ repoRoot });
  const report = evaluateSchemaHygiene({
    inventory,
    config: loadConfig(repoRoot),
  });

  for (const tableName of [
    'application_run_trace_projection_statuses',
    'application_run_trace_nodes',
    'application_run_trace_node_contents',
  ]) {
    const table = report.tables.find((candidate) => candidate.name === tableName);
    assert.equal(
      table.findings.some((finding) => finding.rule === 'managed-table-needs-owner-review'),
      false
    );
    assert.equal(table.platformReadiness.recommendedActions.includes('needs_owner_review'), false);
    assert.equal(table.platformReadiness.category, 'join_or_child');
    assert.equal(table.platformReadiness.scopeGenerationSource.status, 'declared');
    assert.match(table.platformReadiness.scopeGenerationSource.source, /flow_runs\.scope_id/u);
  }
});

test('default schema hygiene config declares issue 1074 owner-confirmed scoped readiness tables', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const inventory = collectSchemaInventory({ repoRoot });
  const report = evaluateSchemaHygiene({
    inventory,
    config: loadConfig(repoRoot),
  });

  for (const tableName of [
    'external_agent_telemetry_events',
    'flow_compiled_plans',
    'flows',
    'mcp_groups',
    'mcp_tool_bindings',
    'model_fields',
  ]) {
    const table = report.tables.find((candidate) => candidate.name === tableName);
    assert.equal(
      table.findings.some((finding) => finding.rule === 'managed-table-needs-owner-review'),
      false
    );
    assert.equal(table.platformReadiness.fields.id.present, true);
    assert.equal(table.platformReadiness.fields.scope_id.present, true);
    assert.equal(table.platformReadiness.fields.created_at.present, true);
    assert.equal(table.platformReadiness.hasScopeTimeIdIndex, true);
    assert.equal(table.platformReadiness.scopeGenerationSource.status, 'declared');
    assert.equal(table.platformReadiness.recommendedActions.includes('needs_owner_review'), false);
  }

  assert.equal(report.tables.some((table) => table.name === 'user_auth_identities'
    && table.findings.some((finding) => finding.rule === 'managed-table-needs-owner-review')), false);
});

test('default schema hygiene config declares issue 1073 lifecycle scoped readiness tables', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const inventory = collectSchemaInventory({ repoRoot });
  const report = evaluateSchemaHygiene({
    inventory,
    config: loadConfig(repoRoot),
  });

  for (const tableName of [
    'application_publication_versions',
    'flow_versions',
    'model_failover_queue_snapshots',
  ]) {
    const table = report.tables.find((candidate) => candidate.name === tableName);
    assert.equal(
      table.findings.some((finding) => finding.rule === 'managed-table-needs-owner-review'),
      false
    );
    assert.equal(table.platformReadiness.fields.id.present, true);
    assert.equal(table.platformReadiness.fields.scope_id.present, true);
    assert.equal(table.platformReadiness.fields.created_at.present, true);
    if (tableName === 'flow_versions') {
      assert.equal(table.platformReadiness.fields.updated_at.present, true);
    }
    assert.equal(table.platformReadiness.hasScopeTimeIdIndex, true);
    assert.equal(table.platformReadiness.scopeGenerationSource.status, 'declared');
    assert.equal(table.platformReadiness.recommendedActions.includes('needs_owner_review'), false);
  }

  assert.equal(report.tables.some((table) => table.name === 'plugin_tasks'
    && table.findings.some((finding) => finding.rule === 'managed-table-needs-owner-review')), false);
});

test('default schema hygiene config declares issue 1085 model provider bounded history profiles', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const inventory = collectSchemaInventory({ repoRoot });
  const report = evaluateSchemaHygiene({
    inventory,
    config: loadConfig(repoRoot),
  });

  for (const [tableName, expectedCategory] of [
    ['model_provider_catalog_entries', 'bounded_catalog_projection'],
    ['model_failover_queue_items', 'bounded_child_list'],
    ['model_catalog_sync_runs', 'low_volume_operational_history'],
    ['model_change_logs', 'low_volume_audit_history'],
  ]) {
    const table = report.tables.find((candidate) => candidate.name === tableName);
    assert.equal(
      table.findings.some((finding) => finding.rule === 'managed-table-needs-owner-review'),
      false
    );
    assert.equal(table.platformReadiness.category, expectedCategory);
    assert.equal(table.platformReadiness.severity, 'ok');
    assert.equal(table.exemption.kind, expectedCategory);
  }

  const changeLogs = report.tables.find((candidate) => candidate.name === 'model_change_logs');
  assert.match(changeLogs.platformReadiness.reason, /data_model_id = null/u);
});

test('default schema hygiene config declares remaining scoped readiness issue tables', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const inventory = collectSchemaInventory({ repoRoot });
  const report = evaluateSchemaHygiene({
    inventory,
    config: loadConfig(repoRoot),
  });

  for (const [tableName, expectedCategory] of [
    ['roles', 'mixed_role_owner_chain'],
    ['role_permissions', 'join_or_child'],
    ['user_role_bindings', 'join_or_child'],
    ['extension_installations', 'system_global'],
    ['plugin_tasks', 'mixed_plugin_task'],
    ['plugin_worker_leases', 'system_runtime'],
    ['data_source_secrets', 'join_or_child'],
    ['data_source_catalog_caches', 'join_or_child'],
    ['audit_logs', 'mixed_audit_history'],
    ['flow_drafts', 'join_or_child'],
  ]) {
    const table = report.tables.find((candidate) => candidate.name === tableName);
    assert.equal(
      table.findings.some((finding) => finding.rule === 'managed-table-needs-owner-review'),
      false,
      `${tableName} should not remain in owner review`
    );
    assert.equal(table.platformReadiness.category, expectedCategory);
    assert.equal(table.platformReadiness.fields.scope_id.present, true);
    assert.equal(table.platformReadiness.fields.created_at.present, true);
    assert.equal(table.platformReadiness.hasScopeTimeIdIndex, true);
    assert.equal(table.platformReadiness.scopeGenerationSource.status, 'declared');
    assert.equal(table.platformReadiness.recommendedActions.includes('needs_owner_review'), false);
  }
});

test('default schema hygiene validates flow-run-owned runtime tables by their real owner key', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const inventory = collectSchemaInventory({ repoRoot });
  const report = evaluateSchemaHygiene({ inventory, config: loadConfig(repoRoot) });

  for (const name of [
    'application_run_trace_refresh_queue',
    'native_trajectory_integrity',
    'provider_protocol_trajectory_events',
    'provider_semantic_trajectory_steps',
  ]) {
    const table = report.tables.find((candidate) => candidate.name === name);
    assert.ok(table, `${name} must resolve from the migration inventory`);
    assert.equal(table.profile, 'flow_run_owned_table');
    assert.deepEqual(table.findings, []);
    assert.equal(table.platformReadiness.hasFlowRunOwnerIndex, true);
  }

  assert.equal(report.tables.find((table) => table.name === 'client_trajectory_captures').profile, 'managed_table');
  assert.equal(report.tables.find((table) => table.name === 'provider_protocol_capsules').profile, 'flow_run_owned_table');
  assert.equal(report.summary.errors, 0);
});

test('specialized archive and content tables retain physical owner keys', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const inventory = collectSchemaInventory({ repoRoot });
  const config = loadConfig(repoRoot);
  const expected = [
    ['client_trajectory_archive_heads', ['request_id'], 'flow_runs', ['flow_run_id']],
    ['client_trajectory_archive_parts', ['part_id'], 'client_trajectory_archive_heads', ['request_id']],
    ['client_trajectory_archive_blocks', ['block_id'], 'client_trajectory_archive_heads', ['request_id']],
    ['client_trajectory_archive_segments', ['block_id'], 'client_trajectory_archive_heads', ['request_id']],
    ['node_run_details', ['node_run_id', 'section'], 'node_runs', ['node_run_id']],
    ['runtime_native_snapshot_items', ['id'], 'applications', ['application_id', 'scope_id']],
    ['runtime_native_snapshot_manifests', ['id'], 'applications', ['application_id', 'scope_id']],
    ['runtime_native_snapshot_references', ['manifest_id', 'item_id'], 'runtime_native_snapshot_manifests', ['manifest_id']],
    ['runtime_observation_body_ownership', ['content_id'], 'runtime_canonical_contents', ['content_id']],
  ];

  for (const [name, primaryKey, owner, ownerColumns] of expected) {
    const table = inventory.tables.find((candidate) => candidate.name === name);
    assert.ok(table, `${name} must remain a physical table`);
    assert.deepEqual(table.primaryKey?.columns, primaryKey, `${name} identity changed`);
    assert.ok(
      table.foreignKeys.some((foreignKey) =>
        foreignKey.references.table === owner
        && ownerColumns.every((column) => foreignKey.columns.includes(column))),
      `${name} must remain owned by ${owner}`
    );
    assert.ok(config.exemptions[name], `${name} needs a documented specialized shape`);
  }
});

test('default schema hygiene preserves retired provider settings as a typed archive', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const inventory = collectSchemaInventory({ repoRoot });
  const report = evaluateSchemaHygiene({ inventory, config: loadConfig(repoRoot) });
  const archive = report.tables.find((table) => table.name === 'retired_host_infrastructure_settings');

  assert.ok(archive, 'retired archive must resolve from the migration inventory');
  assert.equal(inventory.tables.some((table) => table.name === 'host_infrastructure_provider_configs'), false);
  assert.equal(archive.profile, 'retired_archive_table');
  assert.deepEqual(archive.findings, []);
  assert.deepEqual(archive.columns.map((column) => [column.name, column.type]), [
    ['source_table', 'text'],
    ['record', 'jsonb'],
  ]);
  assert.equal(archive.platformReadiness.requiredScopeId, false);
  assert.equal(archive.platformReadiness.severity, 'ok');
  assert.equal(report.summary.errors, 0);
});

test('default schema hygiene config declares issue 1075 system global scoped readiness tables', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const inventory = collectSchemaInventory({ repoRoot });
  const report = evaluateSchemaHygiene({
    inventory,
    config: loadConfig(repoRoot),
  });

  for (const tableName of [
    'file_storages',
    'frontend_block_catalog',
    'host_extension_migrations',
    'js_dependency_registry',
    'node_contribution_registry',
    'permission_definitions',
    'system_default_upgrade_items',
    'system_default_upgrade_runs',
  ]) {
    const table = report.tables.find((candidate) => candidate.name === tableName);
    const scopeColumn = table.columns.find((column) => column.name === 'scope_id');
    assert.equal(
      table.findings.some((finding) => finding.rule === 'managed-table-needs-owner-review'),
      false
    );
    assert.equal(table.platformReadiness.category, 'system_global');
    assert.equal(table.platformReadiness.fields.id.present, true);
    assert.equal(table.platformReadiness.fields.scope_id.present, true);
    assert.equal(scopeColumn.default, true);
    assert.equal(table.checks.some((check) => (
      check.definition.includes('scope_id')
      && check.definition.includes('00000000-0000-0000-0000-000000000000')
    )), true);
    assert.equal(table.platformReadiness.fields.created_at.present, true);
    assert.equal(table.platformReadiness.fields.created_by.present, true);
    assert.equal(table.platformReadiness.fields.updated_by.present, true);
    assert.equal(table.platformReadiness.hasScopeTimeIdIndex, true);
    assert.equal(table.platformReadiness.scopeGenerationSource.status, 'declared');
    assert.match(table.platformReadiness.scopeGenerationSource.source, /SYSTEM_SCOPE_ID/u);
    assert.equal(table.platformReadiness.recommendedActions.includes('needs_owner_review'), false);
  }

  assert.equal(report.tables.some((table) => ['authentication_connections', 'login_entries', 'tenants', 'users', 'workspaces'].includes(table.name)
    && table.findings.some((finding) => finding.rule === 'managed-table-needs-owner-review')), false);
});

test('default schema hygiene config declares network egress system and bounded tables', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const inventory = collectSchemaInventory({ repoRoot });
  const report = evaluateSchemaHygiene({
    inventory,
    config: loadConfig(repoRoot),
  });

  for (const [tableName, expectedCategory, expectedSource] of [
    ['network_egress_providers', 'system_global', /SYSTEM_SCOPE_ID/u],
    ['network_egress_pools', 'system_global', /SYSTEM_SCOPE_ID/u],
    ['network_egress_pool_members', 'bounded_child_list', /network_egress_pools\.scope_id/u],
    ['network_egress_projections', 'bounded_projection', /network_egress_providers\.scope_id/u],
    ['network_egress_provider_secrets', 'bounded_secret_projection', /network_egress_providers\.scope_id/u],
    ['network_egress_routes', 'workspace_routing', /workspace_id/u],
  ]) {
    const table = report.tables.find((candidate) => candidate.name === tableName);
    assert.ok(table, `${tableName} should be present in the schema inventory`);
    assert.deepEqual(table.findings, [], `${tableName} should have no schema hygiene findings`);
    assert.equal(table.platformReadiness.category, expectedCategory);
    assert.equal(table.platformReadiness.severity, 'ok');
    assert.equal(table.platformReadiness.scopeGenerationSource.status, 'declared');
    assert.match(table.platformReadiness.scopeGenerationSource.source, expectedSource);
    assert.equal(table.platformReadiness.recommendedActions.includes('needs_owner_review'), false);
  }

  assert.equal(report.summary.errors, 0);
});

test('default schema hygiene config declares issue 1076 identity join scoped readiness tables', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const inventory = collectSchemaInventory({ repoRoot });
  const report = evaluateSchemaHygiene({
    inventory,
    config: loadConfig(repoRoot),
  });

  for (const tableName of [
    'application_api_mappings',
    'application_environment_variables',
    'application_tag_bindings',
    'frontstage_page_schemas',
    'main_source_defaults',
    'model_provider_instance_secrets',
    'model_provider_main_instances',
    'provider_instance_model_catalog_cache',
  ]) {
    const table = report.tables.find((candidate) => candidate.name === tableName);
    assert.equal(
      table.findings.some((finding) => finding.rule === 'managed-table-needs-owner-review'),
      false
    );
    assert.equal(table.platformReadiness.category, 'join_or_child');
    assert.equal(table.platformReadiness.fields.id.present, true);
    assert.equal(table.platformReadiness.fields.scope_id.present, true);
    assert.equal(table.platformReadiness.fields.created_at.present, true);
    assert.equal(table.platformReadiness.fields.created_by.present, true);
    assert.equal(table.platformReadiness.fields.updated_at.present, true);
    assert.equal(table.platformReadiness.fields.updated_by.present, true);
    assert.equal(table.platformReadiness.hasScopeTimeIdIndex, true);
    assert.equal(table.platformReadiness.scopeGenerationSource.status, 'declared');
    assert.equal(table.platformReadiness.recommendedActions.includes('needs_owner_review'), false);
  }

  assert.equal(report.tables.some((table) => ['role_permissions', 'user_role_bindings'].includes(table.name)
    && table.findings.some((finding) => finding.rule === 'managed-table-needs-owner-review')), false);
});

