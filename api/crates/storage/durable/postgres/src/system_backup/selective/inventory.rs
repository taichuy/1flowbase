use anyhow::{ensure, Result};
use control_plane_contracts::system_backup::selective::{
    SelectiveBackupCategory, SelectiveBackupSelection,
};
use sqlx::{PgConnection, Row};
use std::collections::{BTreeMap, BTreeSet};

// Explicit feature ownership. Persistent execution facts are intentionally included in data.
const GROUPS: &[(&str, &str, &str)] = &[
("applications", "applications application_tags application_tag_bindings flows flow_drafts flow_versions flow_compiled_plans application_environment_variables application_api_mappings application_publication_versions workflow_schedule_triggers workflow_extension_triggers application_extension_sources application_js_dependency_selections", "flow_runs node_runs flow_run_checkpoints flow_run_events flow_run_tool_callback_inbox flow_run_callback_tasks flow_run_callback_resume_attempts flow_run_resume_claims flow_run_recovery_history runtime_spans runtime_events runtime_items runtime_context_projections runtime_usage_ledger runtime_artifacts runtime_audit_hashes runtime_canonical_contents runtime_invocation_context_bindings runtime_debug_artifacts runtime_legacy_shadow_batches runtime_legacy_shadow_rows application_conversations application_conversation_messages application_public_conversations application_run_conversation_message_items application_run_log_summaries application_run_log_tasks application_run_trace_projection_statuses application_run_trace_nodes application_run_trace_node_contents application_run_trace_refresh_queue external_agent_sessions external_agent_telemetry_events gateway_log_conversations gateway_log_turns gateway_log_invocations gateway_log_output_items gateway_log_tool_results provider_protocol_capsules provider_protocol_trajectory_events provider_semantic_trajectory_steps native_trajectory_integrity client_trajectory_captures client_trajectory_steps client_trajectory_sections client_trajectory_node_links assistant_conversations run_archive_upload_sessions run_archive_upload_chunks run_archive_import_jobs run_archive_import_mappings capability_invocations debug_variable_cache_entries"),
("auth-center", "authentication_connections login_entries", ""),
("api-key-authentication", "api_keys api_key_data_model_permissions", ""),
("data-models", "model_definitions model_fields model_definition_versions scope_data_model_grants data_source_instances data_source_secrets main_source_defaults", "model_change_logs data_source_catalog_caches data_source_preview_sessions data_model_side_effect_receipts"),
("files", "file_storages file_tables", "attachments"),
("model-providers", "model_provider_instances model_provider_instance_secrets model_provider_main_instances model_provider_main_model_distribution_rules provider_account_pools model_pricing_rules workspace_billing_settings model_failover_queue_templates model_failover_queue_items model_provider_catalog_sources model_provider_catalog_entries", "model_provider_request_logs provider_instance_model_catalog_cache model_catalog_sync_runs model_failover_queue_snapshots model_failover_attempt_ledger model_provider_preview_sessions runtime_cost_ledger billing_sessions"),
("network-center", "network_egress_providers network_egress_provider_secrets network_egress_pools network_egress_pool_members network_egress_routes network_egress_route_pool_members", "network_egress_projections"),
("mcp-management", "mcp_instances mcp_groups mcp_tools mcp_tool_bindings mcp_meta_tool_configs mcp_instance_discovery_policies mcp_upstream_connections mcp_upstream_connection_secrets mcp_upstream_tool_sources mcp_client_credentials mcp_extension_bundle_imports", ""),
("members", "tenants workspaces users workspace_memberships user_auth_identities user_role_bindings", "user_credit_accounts runtime_credit_ledger credit_event_outbox audit_logs"),
("roles", "roles permission_definitions role_permissions role_data_policies role_data_model_policies role_console_group_policies role_console_operation_policies console_permission_catalog_sync console_permission_catalog_groups console_permission_catalog_operations", "role_console_policy_migration_runs role_console_policy_migration_role_previews role_console_group_policy_snapshots role_console_operation_policy_snapshots role_console_policy_migration_run_artifacts role_console_policy_migration_actor_previews role_console_policy_migration_cutover_state role_console_policy_migration_ledger"),
("i18n-catalog", "i18n_catalog_releases i18n_catalog_release_files i18n_catalog_release_messages i18n_catalog_release_translations workspace_i18n_catalog_states workspace_i18n_catalog_overrides workspace_i18n_catalog_custom_translations workspace_i18n_catalog_obsolete_messages", ""),
("ui-management", "frontstage_pages frontstage_page_tabs frontstage_page_visibility_rules frontstage_block_nodes frontstage_page_schemas frontstage_block_codes frontend_block_catalog ui_code_templates ui_code_template_revisions ui_code_template_defaults ui_component_records ui_component_overrides ui_component_contract_revisions js_dependency_registry workspace_console_settings_orders workspace_console_settings_order_items", "frontstage_executable_upgrade_markers frontstage_executable_upgrade_runs"),
("extension-center", "extension_installations plugin_assignments plugin_contribution_authorizations plugin_contribution_authorization_revisions plugin_settings_template_defaults plugin_settings_template_applications node_contribution_registry plugin_schema_ownership", "plugin_schema_reconcile_receipts plugin_data_idempotency_receipts lifecycle_outbox lifecycle_outbox_deliveries native_plugin_application_requests native_plugin_targets"),
("docs", "", ""), ("backups", "", ""), ("memory-observation", "", ""), ("system-runtime", "host_infrastructure_provider_configs retired_host_infrastructure_settings", "system_default_upgrade_runs system_default_upgrade_items"),
];

pub(super) fn static_owner(table: &str) -> Option<(String, bool)> {
    GROUPS.iter().find_map(|(feature, structure, data)| {
        if structure.split_whitespace().any(|name| name == table) {
            Some((format!("system.{feature}"), false))
        } else if data.split_whitespace().any(|name| name == table) {
            Some((format!("system.{feature}"), true))
        } else {
            None
        }
    })
}
pub(super) fn excluded(table: &str) -> bool {
    matches!(
        table,
        "_sqlx_migrations"
            | "extension_installation_node_inventory"
            | "extension_artifact_instances"
            | "plugin_artifact_instances"
            | "plugin_artifact_cleanup_jobs"
            | "plugin_tasks"
            | "plugin_worker_leases"
            | "plugin_package_catalog_projection"
            | "retained_frontend_module_assets"
            | "host_extension_migrations"
    )
}

#[derive(Debug, Clone)]
pub(super) struct Inventory {
    pub categories: Vec<SelectiveBackupCategory>,
    pub owners: BTreeMap<String, (String, bool)>,
    pub plugins: BTreeMap<String, Vec<String>>,
}
pub(super) async fn inventory(connection: &mut PgConnection) -> Result<Inventory> {
    let rows = sqlx::query("select c.relname, pg_total_relation_size(c.oid)::bigint as bytes from pg_class c join pg_namespace n on n.oid=c.relnamespace where n.nspname=current_schema() and c.relkind in ('r','p') and not c.relispartition")
        .fetch_all(&mut *connection).await?;
    let sizes: BTreeMap<String, i64> = rows
        .into_iter()
        .map(|r| Ok((r.try_get("relname")?, r.try_get("bytes")?)))
        .collect::<Result<_>>()?;
    let mut owners = BTreeMap::new();
    for table in sizes.keys() {
        if let Some(owner) = static_owner(table) {
            owners.insert(table.clone(), owner);
        }
    }
    let mut plugins: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if sizes.contains_key("plugin_schema_ownership") {
        for row in sqlx::query("select distinct physical_table, owner_id from plugin_schema_ownership order by physical_table, owner_id").fetch_all(&mut *connection).await? {
            let name: String = row.try_get("physical_table")?;
            let owner: String = row.try_get("owner_id")?;
            plugins.entry(name.clone()).or_default().push(owner);
            if sizes.contains_key(&name) && !excluded(&name) { owners.entry(name).or_insert(("system.extension-center".into(), true)); }
        }
    }
    if sizes.contains_key("model_definitions") {
        for row in sqlx::query("select d.physical_table_name, d.code, d.scope_kind, d.scope_id, d.owner_kind, d.is_protected, exists(select 1 from file_tables f where f.model_definition_id=d.id) as is_file from model_definitions d where d.source_kind='main_source' and d.data_source_instance_id is null and d.physical_table_name is not null").fetch_all(&mut *connection).await? {
            let name: String = row.try_get("physical_table_name")?;
            let is_builtin = row.try_get::<String,_>("scope_kind")? == "system"
                && row.try_get::<uuid::Uuid,_>("scope_id")? == domain::SYSTEM_SCOPE_ID
                && row.try_get::<String,_>("owner_kind")? == "core"
                && row.try_get::<bool,_>("is_protected")?
                && domain::builtin_data_model_contract(&row.try_get::<String,_>("code")?).is_some();
            if is_builtin { ensure!(static_owner(&name).is_some() || excluded(&name), "builtin table has no explicit settings owner: {name}"); continue; }
            if sizes.contains_key(&name) && !excluded(&name) {
                let feature = if row.try_get::<bool,_>("is_file")? { "system.files" } else { "system.data-models" };
                owners.entry(name).or_insert((feature.into(), true));
            }
        }
    }
    let unknown: Vec<_> = sizes
        .keys()
        .filter(|table| !owners.contains_key(*table) && !excluded(table))
        .cloned()
        .collect();
    ensure!(
        unknown.is_empty(),
        "unmapped persistent settings tables: {}",
        unknown.join(", ")
    );
    let categories = GROUPS
        .iter()
        .map(|(feature, _, _)| {
            let feature_id = format!("system.{feature}");
            let mut category = SelectiveBackupCategory {
                feature_id: feature_id.clone(),
                label_key: label(feature).into(),
                structure_bytes: 0,
                data_bytes: 0,
                structure_tables: vec![],
                data_tables: vec![],
            };
            for (table, (owner, data)) in &owners {
                if owner == &feature_id {
                    let bytes = sizes.get(table).copied().unwrap_or(0).max(0) as u64;
                    if *data {
                        category.data_tables.push(table.clone());
                        category.data_bytes += bytes;
                    } else {
                        category.structure_tables.push(table.clone());
                        category.structure_bytes += bytes;
                    }
                }
            }
            category
        })
        .collect();
    Ok(Inventory {
        categories,
        owners,
        plugins,
    })
}
pub(super) fn selected(
    inv: &Inventory,
    selection: &[SelectiveBackupSelection],
) -> Result<BTreeSet<String>> {
    ensure!(
        !selection.is_empty(),
        "select at least one settings feature"
    );
    let mut seen = BTreeSet::new();
    let mut tables = BTreeSet::new();
    for item in selection {
        ensure!(
            seen.insert(item.feature_id.clone()),
            "duplicate feature selection"
        );
        let category = inv
            .categories
            .iter()
            .find(|c| c.feature_id == item.feature_id)
            .ok_or_else(|| anyhow::anyhow!("unknown settings feature: {}", item.feature_id))?;
        if item.structure {
            tables.extend(category.structure_tables.iter().cloned());
        }
        if item.data {
            tables.extend(category.data_tables.iter().cloned());
        }
    }
    ensure!(
        !tables.is_empty(),
        "selection contains no persisted settings rows"
    );
    Ok(tables)
}

fn label(feature: &str) -> &'static str {
    match feature {
        "applications" => "auto.application_management",
        "auth-center" => "auto.auth_center",
        "api-key-authentication" => "auto.api_key_authentication",
        "data-models" => "auto.data_source",
        "files" => "auto.file_management",
        "model-providers" => "auto.model_providers",
        "network-center" => "auto.network_center",
        "mcp-management" => "auto.mcp_management",
        "members" => "auto.user_management",
        "roles" => "auto.permission_management",
        "i18n-catalog" => "auto.translation_catalog_title",
        "ui-management" => "auto.ui_management",
        "extension-center" => "auto.extension_center",
        "docs" => "auto.api_documentation",
        "backups" => "auto.backups",
        "memory-observation" => "auto.memory_observation",
        _ => "auto.system_runtime",
    }
}
