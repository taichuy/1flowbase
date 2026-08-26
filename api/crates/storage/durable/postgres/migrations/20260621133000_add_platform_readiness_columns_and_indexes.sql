alter table application_js_dependency_selections add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table application_js_dependency_selections alter column scope_id set not null;
alter table application_js_dependency_selections add column if not exists created_by uuid;
alter table application_js_dependency_selections add column if not exists updated_by uuid;
create index if not exists application_js_dependency_selections_scope_created_id_idx on application_js_dependency_selections (scope_id, created_at, id);

alter table application_tags add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table application_tags alter column scope_id set not null;
alter table application_tags add column if not exists created_by uuid;
alter table application_tags add column if not exists updated_by uuid;
create index if not exists application_tags_scope_created_id_idx on application_tags (scope_id, created_at, id);

alter table applications add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table applications alter column scope_id set not null;
create index if not exists applications_scope_created_id_idx on applications (scope_id, created_at, id);

alter table billing_sessions add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table billing_sessions alter column scope_id set not null;
alter table billing_sessions add column if not exists created_by uuid;
alter table billing_sessions add column if not exists updated_by uuid;
create index if not exists billing_sessions_scope_created_id_idx on billing_sessions (scope_id, created_at, id);

alter table data_source_instances add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table data_source_instances alter column scope_id set not null;
alter table data_source_instances add column if not exists updated_by uuid;
create index if not exists data_source_instances_scope_created_id_idx on data_source_instances (scope_id, created_at, id);

alter table data_source_preview_sessions add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table data_source_preview_sessions alter column scope_id set not null;
alter table data_source_preview_sessions add column if not exists updated_at timestamptz not null default now();
alter table data_source_preview_sessions add column if not exists created_by uuid;
alter table data_source_preview_sessions add column if not exists updated_by uuid;
create index if not exists data_source_preview_sessions_scope_created_id_idx on data_source_preview_sessions (scope_id, created_at, id);

alter table debug_variable_cache_entries add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table debug_variable_cache_entries alter column scope_id set not null;
alter table debug_variable_cache_entries add column if not exists created_by uuid;
alter table debug_variable_cache_entries add column if not exists updated_by uuid;
create index if not exists debug_variable_cache_entries_scope_created_id_idx on debug_variable_cache_entries (scope_id, created_at, id);

alter table external_agent_sessions add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table external_agent_sessions alter column scope_id set not null;
alter table external_agent_sessions add column if not exists updated_at timestamptz not null default now();
alter table external_agent_sessions add column if not exists created_by uuid;
alter table external_agent_sessions add column if not exists updated_by uuid;
create index if not exists external_agent_sessions_scope_created_id_idx on external_agent_sessions (scope_id, created_at, id);

alter table frontstage_block_codes add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table frontstage_block_codes alter column scope_id set not null;
alter table frontstage_block_codes add column if not exists created_by uuid;
alter table frontstage_block_codes add column if not exists updated_by uuid;
create index if not exists frontstage_block_codes_scope_created_id_idx on frontstage_block_codes (scope_id, created_at, id);

alter table frontstage_pages add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table frontstage_pages alter column scope_id set not null;
alter table frontstage_pages add column if not exists created_by uuid;
alter table frontstage_pages add column if not exists updated_by uuid;
create index if not exists frontstage_pages_scope_created_id_idx on frontstage_pages (scope_id, created_at, id);

alter table mcp_instances add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table mcp_instances alter column scope_id set not null;
create index if not exists mcp_instances_scope_created_id_idx on mcp_instances (scope_id, created_at, id);

alter table mcp_meta_tool_configs add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table mcp_meta_tool_configs alter column scope_id set not null;
create index if not exists mcp_meta_tool_configs_scope_created_id_idx on mcp_meta_tool_configs (scope_id, created_at, id);

alter table mcp_tools add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table mcp_tools alter column scope_id set not null;
create index if not exists mcp_tools_scope_created_id_idx on mcp_tools (scope_id, created_at, id);

alter table model_failover_queue_templates add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table model_failover_queue_templates alter column scope_id set not null;
alter table model_failover_queue_templates add column if not exists updated_by uuid;
create index if not exists model_failover_queue_templates_scope_created_id_idx on model_failover_queue_templates (scope_id, created_at, id);

alter table model_provider_catalog_sources add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table model_provider_catalog_sources alter column scope_id set not null;
alter table model_provider_catalog_sources add column if not exists created_by uuid;
alter table model_provider_catalog_sources add column if not exists updated_by uuid;
create index if not exists model_provider_catalog_sources_scope_created_id_idx on model_provider_catalog_sources (scope_id, created_at, id);

alter table model_provider_instances add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table model_provider_instances alter column scope_id set not null;
create index if not exists model_provider_instances_scope_created_id_idx on model_provider_instances (scope_id, created_at, id);

alter table model_provider_preview_sessions add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table model_provider_preview_sessions alter column scope_id set not null;
alter table model_provider_preview_sessions add column if not exists updated_at timestamptz not null default now();
alter table model_provider_preview_sessions add column if not exists created_by uuid;
alter table model_provider_preview_sessions add column if not exists updated_by uuid;
create index if not exists model_provider_preview_sessions_scope_created_id_idx on model_provider_preview_sessions (scope_id, created_at, id);

alter table plugin_assignments add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table plugin_assignments alter column scope_id set not null;
alter table plugin_assignments add column if not exists updated_at timestamptz not null default now();
alter table plugin_assignments add column if not exists created_by uuid;
alter table plugin_assignments add column if not exists updated_by uuid;
create index if not exists plugin_assignments_scope_created_id_idx on plugin_assignments (scope_id, created_at, id);

alter table provider_account_pools add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table provider_account_pools alter column scope_id set not null;
alter table provider_account_pools add column if not exists created_by uuid;
alter table provider_account_pools add column if not exists updated_by uuid;
create index if not exists provider_account_pools_scope_created_id_idx on provider_account_pools (scope_id, created_at, id);

alter table runtime_cost_ledger add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table runtime_cost_ledger alter column scope_id set not null;
alter table runtime_cost_ledger add column if not exists created_by uuid;
alter table runtime_cost_ledger add column if not exists updated_by uuid;
create index if not exists runtime_cost_ledger_scope_created_id_idx on runtime_cost_ledger (scope_id, created_at, id);

alter table runtime_credit_ledger add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table runtime_credit_ledger alter column scope_id set not null;
alter table runtime_credit_ledger add column if not exists created_by uuid;
alter table runtime_credit_ledger add column if not exists updated_by uuid;
create index if not exists runtime_credit_ledger_scope_created_id_idx on runtime_credit_ledger (scope_id, created_at, id);

alter table runtime_debug_artifacts add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table runtime_debug_artifacts alter column scope_id set not null;
alter table runtime_debug_artifacts add column if not exists created_by uuid;
alter table runtime_debug_artifacts add column if not exists updated_by uuid;
create index if not exists runtime_debug_artifacts_scope_created_id_idx on runtime_debug_artifacts (scope_id, created_at, id);

alter table workspace_memberships add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table workspace_memberships alter column scope_id set not null;
create index if not exists workspace_memberships_scope_created_id_idx on workspace_memberships (scope_id, created_at, id);

alter table api_keys add column if not exists created_by uuid;
alter table api_keys add column if not exists updated_by uuid;
create index if not exists api_keys_scope_created_id_idx on api_keys (scope_id, created_at, id);

alter table application_run_log_summaries add column if not exists created_by uuid;
alter table application_run_log_summaries add column if not exists updated_by uuid;
create index if not exists application_run_log_summaries_scope_created_id_idx on application_run_log_summaries (scope_id, created_at, id);

alter table data_model_side_effect_receipts add column if not exists updated_at timestamptz not null default now();
alter table data_model_side_effect_receipts add column if not exists created_by uuid;
alter table data_model_side_effect_receipts add column if not exists updated_by uuid;
create index if not exists data_model_side_effect_receipts_scope_created_id_idx on data_model_side_effect_receipts (scope_id, created_at, id);

create index if not exists file_tables_scope_created_id_idx on file_tables (scope_id, created_at, id);

alter table flow_run_callback_resume_attempts add column if not exists created_by uuid;
alter table flow_run_callback_resume_attempts add column if not exists updated_by uuid;
create index if not exists flow_run_callback_resume_attempts_scope_created_id_idx on flow_run_callback_resume_attempts (scope_id, created_at, id);

alter table flow_run_callback_tasks add column if not exists created_by uuid;
alter table flow_run_callback_tasks add column if not exists updated_by uuid;
create index if not exists flow_run_callback_tasks_scope_created_id_idx on flow_run_callback_tasks (scope_id, created_at, id);

alter table flow_run_checkpoints add column if not exists created_by uuid;
alter table flow_run_checkpoints add column if not exists updated_by uuid;
create index if not exists flow_run_checkpoints_scope_created_id_idx on flow_run_checkpoints (scope_id, created_at, id);

alter table flow_run_events add column if not exists created_by uuid;
alter table flow_run_events add column if not exists updated_by uuid;
create index if not exists flow_run_events_scope_created_id_idx on flow_run_events (scope_id, created_at, id);

create index if not exists model_definitions_scope_created_id_idx on model_definitions (scope_id, created_at, id);

alter table node_runs add column if not exists created_by uuid;
alter table node_runs add column if not exists updated_by uuid;
create index if not exists node_runs_scope_created_id_idx on node_runs (scope_id, created_at, id);

alter table scope_data_model_grants add column if not exists updated_by uuid;
create index if not exists scope_data_model_grants_scope_created_id_idx on scope_data_model_grants (scope_id, created_at, id);

alter table application_conversations add column if not exists created_by uuid;
alter table application_conversations add column if not exists updated_by uuid;

alter table application_conversation_messages add column if not exists created_by uuid;
alter table application_conversation_messages add column if not exists updated_by uuid;

create or replace function set_scope_id_from_application_id()
returns trigger
language plpgsql
as $$
begin
  if new.scope_id is null then
    select applications.workspace_id
      into new.scope_id
      from applications
     where applications.id = new.application_id;
  end if;
  return new;
end;
$$;

create or replace function set_scope_id_from_flow_run_id()
returns trigger
language plpgsql
as $$
begin
  if new.scope_id is null then
    select flow_runs.scope_id
      into new.scope_id
      from flow_runs
     where flow_runs.id = new.flow_run_id;
  end if;
  return new;
end;
$$;

alter table flow_runs add column if not exists scope_id uuid;
alter table flow_runs add column if not exists updated_by uuid;
update flow_runs
   set scope_id = applications.workspace_id
  from applications
 where flow_runs.application_id = applications.id
   and flow_runs.scope_id is null;
alter table flow_runs alter column scope_id set not null;
drop trigger if exists flow_runs_set_scope_id_from_application_id on flow_runs;
create trigger flow_runs_set_scope_id_from_application_id
before insert or update of application_id, scope_id on flow_runs
for each row execute function set_scope_id_from_application_id();
create index if not exists flow_runs_scope_created_id_idx on flow_runs (scope_id, created_at, id);
create index if not exists flow_runs_scope_started_id_idx on flow_runs (scope_id, started_at, id);
create index if not exists flow_runs_scope_updated_id_idx on flow_runs (scope_id, updated_at, id);

alter table runtime_spans add column if not exists scope_id uuid;
alter table runtime_spans add column if not exists created_at timestamptz;
alter table runtime_spans add column if not exists updated_at timestamptz not null default now();
alter table runtime_spans add column if not exists created_by uuid;
alter table runtime_spans add column if not exists updated_by uuid;
update runtime_spans
   set scope_id = flow_runs.scope_id
  from flow_runs
 where runtime_spans.flow_run_id = flow_runs.id
   and runtime_spans.scope_id is null;
update runtime_spans
   set created_at = started_at
 where created_at is null;
alter table runtime_spans alter column scope_id set not null;
alter table runtime_spans alter column created_at set default now();
alter table runtime_spans alter column created_at set not null;
drop trigger if exists runtime_spans_set_scope_id_from_flow_run_id on runtime_spans;
create trigger runtime_spans_set_scope_id_from_flow_run_id
before insert or update of flow_run_id, scope_id on runtime_spans
for each row execute function set_scope_id_from_flow_run_id();
create index if not exists runtime_spans_scope_created_id_idx on runtime_spans (scope_id, created_at, id);
create index if not exists runtime_spans_scope_flow_started_id_idx on runtime_spans (scope_id, flow_run_id, started_at, id);

alter table runtime_events add column if not exists scope_id uuid;
alter table runtime_events add column if not exists created_by uuid;
alter table runtime_events add column if not exists updated_by uuid;
update runtime_events
   set scope_id = flow_runs.scope_id
  from flow_runs
 where runtime_events.flow_run_id = flow_runs.id
   and runtime_events.scope_id is null;
alter table runtime_events alter column scope_id set not null;
drop trigger if exists runtime_events_set_scope_id_from_flow_run_id on runtime_events;
create trigger runtime_events_set_scope_id_from_flow_run_id
before insert or update of flow_run_id, scope_id on runtime_events
for each row execute function set_scope_id_from_flow_run_id();
create index if not exists runtime_events_scope_created_id_idx on runtime_events (scope_id, created_at, id);
create index if not exists runtime_events_scope_flow_sequence_id_idx on runtime_events (scope_id, flow_run_id, sequence, id);

alter table runtime_items add column if not exists scope_id uuid;
alter table runtime_items add column if not exists created_by uuid;
alter table runtime_items add column if not exists updated_by uuid;
update runtime_items
   set scope_id = flow_runs.scope_id
  from flow_runs
 where runtime_items.flow_run_id = flow_runs.id
   and runtime_items.scope_id is null;
alter table runtime_items alter column scope_id set not null;
drop trigger if exists runtime_items_set_scope_id_from_flow_run_id on runtime_items;
create trigger runtime_items_set_scope_id_from_flow_run_id
before insert or update of flow_run_id, scope_id on runtime_items
for each row execute function set_scope_id_from_flow_run_id();
create index if not exists runtime_items_scope_created_id_idx on runtime_items (scope_id, created_at, id);
create index if not exists runtime_items_scope_flow_created_id_idx on runtime_items (scope_id, flow_run_id, created_at, id);

alter table runtime_context_projections add column if not exists scope_id uuid;
alter table runtime_context_projections add column if not exists created_by uuid;
alter table runtime_context_projections add column if not exists updated_by uuid;
update runtime_context_projections
   set scope_id = flow_runs.scope_id
  from flow_runs
 where runtime_context_projections.flow_run_id = flow_runs.id
   and runtime_context_projections.scope_id is null;
alter table runtime_context_projections alter column scope_id set not null;
drop trigger if exists runtime_context_projections_set_scope_id_from_flow_run_id on runtime_context_projections;
create trigger runtime_context_projections_set_scope_id_from_flow_run_id
before insert or update of flow_run_id, scope_id on runtime_context_projections
for each row execute function set_scope_id_from_flow_run_id();
create index if not exists runtime_context_projections_scope_created_id_idx on runtime_context_projections (scope_id, created_at, id);
create index if not exists runtime_context_projections_scope_flow_created_id_idx on runtime_context_projections (scope_id, flow_run_id, created_at, id);

alter table runtime_usage_ledger add column if not exists scope_id uuid;
alter table runtime_usage_ledger add column if not exists created_by uuid;
alter table runtime_usage_ledger add column if not exists updated_by uuid;
update runtime_usage_ledger
   set scope_id = flow_runs.scope_id
  from flow_runs
 where runtime_usage_ledger.flow_run_id = flow_runs.id
   and runtime_usage_ledger.scope_id is null;
alter table runtime_usage_ledger alter column scope_id set not null;
drop trigger if exists runtime_usage_ledger_set_scope_id_from_flow_run_id on runtime_usage_ledger;
create trigger runtime_usage_ledger_set_scope_id_from_flow_run_id
before insert or update of flow_run_id, scope_id on runtime_usage_ledger
for each row execute function set_scope_id_from_flow_run_id();
create index if not exists runtime_usage_ledger_scope_created_id_idx on runtime_usage_ledger (scope_id, created_at, id);
create index if not exists runtime_usage_ledger_scope_flow_created_id_idx on runtime_usage_ledger (scope_id, flow_run_id, created_at, id);

alter table runtime_artifacts add column if not exists scope_id uuid;
alter table runtime_artifacts add column if not exists created_by uuid;
alter table runtime_artifacts add column if not exists updated_by uuid;
update runtime_artifacts
   set scope_id = flow_runs.scope_id
  from flow_runs
 where runtime_artifacts.flow_run_id = flow_runs.id
   and runtime_artifacts.scope_id is null;
alter table runtime_artifacts alter column scope_id set not null;
drop trigger if exists runtime_artifacts_set_scope_id_from_flow_run_id on runtime_artifacts;
create trigger runtime_artifacts_set_scope_id_from_flow_run_id
before insert or update of flow_run_id, scope_id on runtime_artifacts
for each row execute function set_scope_id_from_flow_run_id();
create index if not exists runtime_artifacts_scope_created_id_idx on runtime_artifacts (scope_id, created_at, id);
create index if not exists runtime_artifacts_scope_flow_created_id_idx on runtime_artifacts (scope_id, flow_run_id, created_at, id);

alter table runtime_audit_hashes add column if not exists scope_id uuid;
alter table runtime_audit_hashes add column if not exists created_by uuid;
alter table runtime_audit_hashes add column if not exists updated_by uuid;
update runtime_audit_hashes
   set scope_id = flow_runs.scope_id
  from flow_runs
 where runtime_audit_hashes.flow_run_id = flow_runs.id
   and runtime_audit_hashes.scope_id is null;
alter table runtime_audit_hashes alter column scope_id set not null;
drop trigger if exists runtime_audit_hashes_set_scope_id_from_flow_run_id on runtime_audit_hashes;
create trigger runtime_audit_hashes_set_scope_id_from_flow_run_id
before insert or update of flow_run_id, scope_id on runtime_audit_hashes
for each row execute function set_scope_id_from_flow_run_id();
create index if not exists runtime_audit_hashes_scope_created_id_idx on runtime_audit_hashes (scope_id, created_at, id);
create index if not exists runtime_audit_hashes_scope_flow_created_id_idx on runtime_audit_hashes (scope_id, flow_run_id, created_at, id);

alter table capability_invocations add column if not exists scope_id uuid;
alter table capability_invocations add column if not exists updated_at timestamptz not null default now();
alter table capability_invocations add column if not exists created_by uuid;
alter table capability_invocations add column if not exists updated_by uuid;
update capability_invocations
   set scope_id = flow_runs.scope_id
  from flow_runs
 where capability_invocations.flow_run_id = flow_runs.id
   and capability_invocations.scope_id is null;
alter table capability_invocations alter column scope_id set not null;
drop trigger if exists capability_invocations_set_scope_id_from_flow_run_id on capability_invocations;
create trigger capability_invocations_set_scope_id_from_flow_run_id
before insert or update of flow_run_id, scope_id on capability_invocations
for each row execute function set_scope_id_from_flow_run_id();
create index if not exists capability_invocations_scope_created_id_idx on capability_invocations (scope_id, created_at, id);
create index if not exists capability_invocations_scope_flow_started_id_idx on capability_invocations (scope_id, flow_run_id, started_at, id);

alter table model_failover_attempt_ledger add column if not exists scope_id uuid;
alter table model_failover_attempt_ledger add column if not exists created_at timestamptz;
alter table model_failover_attempt_ledger add column if not exists updated_at timestamptz not null default now();
alter table model_failover_attempt_ledger add column if not exists created_by uuid;
alter table model_failover_attempt_ledger add column if not exists updated_by uuid;
update model_failover_attempt_ledger
   set scope_id = flow_runs.scope_id
  from flow_runs
 where model_failover_attempt_ledger.flow_run_id = flow_runs.id
   and model_failover_attempt_ledger.scope_id is null;
update model_failover_attempt_ledger
   set created_at = started_at
 where created_at is null;
alter table model_failover_attempt_ledger alter column scope_id set not null;
alter table model_failover_attempt_ledger alter column created_at set default now();
alter table model_failover_attempt_ledger alter column created_at set not null;
drop trigger if exists model_failover_attempt_ledger_set_scope_id_from_flow_run_id on model_failover_attempt_ledger;
create trigger model_failover_attempt_ledger_set_scope_id_from_flow_run_id
before insert or update of flow_run_id, scope_id on model_failover_attempt_ledger
for each row execute function set_scope_id_from_flow_run_id();
create index if not exists model_failover_attempt_ledger_scope_created_id_idx on model_failover_attempt_ledger (scope_id, created_at, id);

alter table application_public_conversations add column if not exists scope_id uuid;
alter table application_public_conversations add column if not exists created_by uuid;
alter table application_public_conversations add column if not exists updated_by uuid;
update application_public_conversations
   set scope_id = applications.workspace_id
  from applications
 where application_public_conversations.application_id = applications.id
   and application_public_conversations.scope_id is null;
alter table application_public_conversations alter column scope_id set not null;
drop trigger if exists application_public_conversations_set_scope_id_from_application_id on application_public_conversations;
create trigger application_public_conversations_set_scope_id_from_application_id
before insert or update of application_id, scope_id on application_public_conversations
for each row execute function set_scope_id_from_application_id();
create index if not exists application_public_conversations_scope_created_id_idx on application_public_conversations (scope_id, created_at, id);
