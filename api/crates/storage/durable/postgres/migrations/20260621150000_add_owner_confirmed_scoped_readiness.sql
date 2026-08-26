alter table flows add column if not exists scope_id uuid;
update flows
   set scope_id = applications.scope_id
  from applications
 where flows.application_id = applications.id
   and flows.scope_id is null;
alter table flows alter column scope_id set not null;
create index if not exists flows_scope_created_id_idx on flows (scope_id, created_at, id);

alter table flow_compiled_plans add column if not exists scope_id uuid;
alter table flow_compiled_plans add column if not exists updated_by uuid;
update flow_compiled_plans
   set scope_id = flows.scope_id,
       updated_by = coalesce(flow_compiled_plans.updated_by, flow_compiled_plans.created_by)
  from flows
 where flow_compiled_plans.flow_id = flows.id
   and flow_compiled_plans.scope_id is null;
alter table flow_compiled_plans alter column scope_id set not null;
create index if not exists flow_compiled_plans_scope_created_id_idx on flow_compiled_plans (scope_id, created_at, id);

alter table external_agent_telemetry_events add column if not exists scope_id uuid;
alter table external_agent_telemetry_events add column if not exists created_by uuid;
alter table external_agent_telemetry_events add column if not exists updated_by uuid;
update external_agent_telemetry_events events
   set scope_id = sessions.scope_id
  from external_agent_sessions sessions
 where events.external_agent_session_id = sessions.id
   and events.scope_id is null;
alter table external_agent_telemetry_events alter column scope_id set not null;
create index if not exists external_agent_telemetry_events_scope_created_id_idx on external_agent_telemetry_events (scope_id, created_at, id);

create or replace function set_scope_id_from_external_agent_session_id()
returns trigger
language plpgsql
as $$
begin
  if new.scope_id is null then
    select external_agent_sessions.scope_id
      into new.scope_id
      from external_agent_sessions
     where external_agent_sessions.id = new.external_agent_session_id;
  end if;
  return new;
end;
$$;

drop trigger if exists external_agent_telemetry_events_set_scope_id_from_session on external_agent_telemetry_events;
create trigger external_agent_telemetry_events_set_scope_id_from_session
before insert or update of external_agent_session_id, scope_id on external_agent_telemetry_events
for each row execute function set_scope_id_from_external_agent_session_id();
comment on trigger external_agent_telemetry_events_set_scope_id_from_session on external_agent_telemetry_events
  is 'Temporary scoped-readiness bridge for append-only telemetry rows; replace with repository-owned scope_id writes when the telemetry write path is introduced by 2026-09-30.';

alter table mcp_groups add column if not exists scope_id uuid;
update mcp_groups
   set scope_id = mcp_instances.scope_id
  from mcp_instances
 where mcp_groups.instance_record_id = mcp_instances.id
   and mcp_groups.scope_id is null;
alter table mcp_groups alter column scope_id set not null;
create index if not exists mcp_groups_scope_created_id_idx on mcp_groups (scope_id, created_at, id);

alter table mcp_tool_bindings add column if not exists scope_id uuid;
update mcp_tool_bindings
   set scope_id = mcp_instances.scope_id
  from mcp_instances
 where mcp_tool_bindings.instance_record_id = mcp_instances.id
   and mcp_tool_bindings.scope_id is null;
alter table mcp_tool_bindings alter column scope_id set not null;
create index if not exists mcp_tool_bindings_scope_created_id_idx on mcp_tool_bindings (scope_id, created_at, id);

alter table model_fields add column if not exists scope_id uuid;
update model_fields
   set scope_id = model_definitions.scope_id
  from model_definitions
 where model_fields.data_model_id = model_definitions.id
   and model_fields.scope_id is null;
alter table model_fields alter column scope_id set not null;
create index if not exists model_fields_scope_created_id_idx on model_fields (scope_id, created_at, id);
