alter table role_console_group_policies
  add column enabled boolean,
  add column strategy text;

update role_console_group_policies
set enabled = mode <> 'disabled',
    strategy = case when mode = 'custom' then 'custom' else 'full' end;

alter table role_console_group_policies
  alter column enabled set not null,
  alter column strategy set not null,
  add constraint role_console_group_policies_strategy_check
    check (strategy in ('full', 'custom'));

alter table role_console_operation_policies
  drop constraint role_console_operation_policies_group_fk,
  drop constraint role_console_operation_policies_group_mode_check,
  add constraint role_console_operation_policies_group_fk
    foreign key (group_policy_id, role_id)
    references role_console_group_policies (id, role_id)
    on delete cascade;

alter table role_console_group_policies
  drop constraint role_console_group_policies_id_role_mode_key,
  drop constraint role_console_group_policies_mode_check,
  drop column mode;

alter table role_console_operation_policies
  drop column group_mode;

alter table role_console_group_policy_snapshots
  add column enabled boolean,
  add column strategy text;

update role_console_group_policy_snapshots
set enabled = mode <> 'disabled',
    strategy = case when mode = 'custom' then 'custom' else 'full' end;

alter table role_console_group_policy_snapshots
  alter column enabled set not null,
  alter column strategy set not null;

create temporary table console_operation_profile_expansion (
  profile_id text primary key,
  operation_ids text[] not null
) on commit drop;

insert into console_operation_profile_expansion values
  ('applications.create', ARRAY['create_application','create_application_tag','get_application_catalog']),
  ('applications.logs.export', ARRAY['export_application_run_archive','export_application_run_trace_dump','export_application_runs_archive','export_application_runs_zip']),
  ('applications.logs.import', ARRAY['complete_run_archive_upload_session','create_run_archive_upload_session','get_run_archive_import_job','upload_run_archive_chunk']),
  ('applications.publish', ARRAY['publish_application_api','unpublish_application_api']),
  ('applications.run', ARRAY['cancel_flow_run','complete_callback_task','resume_flow_run','start_flow_debug_run','start_flow_debug_run_stream','start_node_debug_preview']),
  ('applications.update', ARRAY['create_application_api_key','delete_debug_variable_cache_entries','patch_application','replace_application_api_mapping','replace_application_environment_variables','replace_application_js_dependency_selection','replace_workflow_schedule_trigger','revoke_application_api_key','save_draft','update_version','upsert_debug_variable_cache_entry']),
  ('applications.view', ARRAY['get_application','get_application_api_docs_catalog','get_application_api_docs_category_openapi','get_application_api_docs_category_operations','get_application_api_docs_operation_openapi','get_application_api_mapping','get_application_api_publication','get_application_operation_bindings','get_application_run_monitoring_report','get_application_run_node_last_run','get_application_run_overview','get_application_run_resume_timeline','get_application_run_trace_node_children','get_application_run_trace_node_content','get_application_run_trace_node_detail','get_application_run_trace_tool_callback_content','get_application_run_trace_tree','get_application_runtime_activity','get_debug_variable_snapshot','get_flow_debug_run_snapshot','get_node_last_run','get_orchestration','get_runtime_debug_artifact','get_runtime_debug_stream','get_workflow_schedule_trigger','list_application_api_keys','list_application_conversation_messages','list_application_environment_variables','list_application_js_dependency_selections','list_application_run_conversation_messages','list_application_runs','list_applications','resolve_runtime_debug_artifacts','subscribe_flow_debug_run_stream']),
  ('auth_center.authenticators.update', ARRAY['update_auth_center_authenticator_config','update_auth_center_authenticator_public_ui_block']),
  ('host_infrastructure.cache.view', ARRAY['get_host_infrastructure_cache_overview','list_host_infrastructure_cache_entries']),
  ('host_infrastructure.memory.view', ARRAY['get_host_infrastructure_memory_overview','get_host_infrastructure_memory_stats','get_host_infrastructure_memory_stats_overview','list_host_infrastructure_memory_entries','list_host_infrastructure_memory_tree','search_host_infrastructure_memory_entries']),
  ('mcp.bundles.export', ARRAY['export_mcp_bundle','get_mcp_bundle_export_defaults']),
  ('mcp.bundles.import', ARRAY['import_official_mcp_bundle','import_uploaded_mcp_bundle']),
  ('mcp.bundles.preview', ARRAY['preview_official_mcp_bundle','preview_uploaded_mcp_bundle']),
  ('mcp.catalog.view', ARRAY['get_mcp_catalog','list_mcp_interface_capabilities','list_mcp_items']),
  ('mcp.tools.view', ARRAY['get_mcp_tool','list_mcp_tools']),
  ('mcp.upstream_connections.test', ARRAY['test_connection','test_draft_connection']),
  ('model_definitions.delete', ARRAY['batch_delete_models','delete_model']),
  ('plugins.tasks.view', ARRAY['plugin_get_task','plugin_list_tasks']),
  ('settings_feature.access.system.docs', ARRAY['get_console_docs_catalog','get_console_docs_category_openapi','get_console_docs_operation_openapi','list_console_docs_category_operations']),
  ('user_api_keys.manage', ARRAY['create_user_api_key','list_user_api_key_role_options','list_user_api_keys','revoke_user_api_key']);

insert into role_console_operation_policies (
  id, role_id, group_policy_id, operation_id, policy_kind,
  simple_enabled, row_scope, created_by, created_at, updated_by, updated_at
)
select gen_random_uuid(), policy.role_id, policy.group_policy_id, expanded_operation.operation_id,
       policy.policy_kind, policy.simple_enabled, policy.row_scope,
       policy.created_by, policy.created_at, policy.updated_by, policy.updated_at
from role_console_operation_policies policy
join console_operation_profile_expansion expansion on expansion.profile_id = policy.operation_id
cross join lateral unnest(expansion.operation_ids) as expanded_operation(operation_id);

delete from role_console_operation_policies policy
using console_operation_profile_expansion expansion
where policy.operation_id = expansion.profile_id;

insert into role_console_operation_policy_snapshots (
  run_id, operation_policy_id, role_id, group_policy_id, group_mode,
  operation_id, policy_kind, simple_enabled, row_scope,
  created_by, created_at, updated_by, updated_at
)
select snapshot.run_id, gen_random_uuid(), snapshot.role_id, snapshot.group_policy_id,
       snapshot.group_mode, expanded_operation.operation_id, snapshot.policy_kind,
       snapshot.simple_enabled, snapshot.row_scope, snapshot.created_by,
       snapshot.created_at, snapshot.updated_by, snapshot.updated_at
from role_console_operation_policy_snapshots snapshot
join console_operation_profile_expansion expansion on expansion.profile_id = snapshot.operation_id
cross join lateral unnest(expansion.operation_ids) as expanded_operation(operation_id);

delete from role_console_operation_policy_snapshots snapshot
using console_operation_profile_expansion expansion
where snapshot.operation_id = expansion.profile_id;
