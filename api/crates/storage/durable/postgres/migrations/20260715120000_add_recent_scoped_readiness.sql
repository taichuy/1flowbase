alter table frontstage_page_tabs
  add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table frontstage_page_tabs alter column scope_id set not null;
create index if not exists frontstage_page_tabs_scope_created_id_idx
  on frontstage_page_tabs (scope_id, created_at, id);

alter table frontstage_page_visibility_rules
  add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table frontstage_page_visibility_rules alter column scope_id set not null;
create index if not exists frontstage_page_visibility_rules_scope_created_id_idx
  on frontstage_page_visibility_rules (scope_id, created_at, id);

alter table mcp_client_credentials
  add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table mcp_client_credentials alter column scope_id set not null;
create index if not exists mcp_client_credentials_scope_created_id_idx
  on mcp_client_credentials (scope_id, created_at, id);

alter table mcp_upstream_connections
  add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table mcp_upstream_connections alter column scope_id set not null;
create index if not exists mcp_upstream_connections_scope_created_id_idx
  on mcp_upstream_connections (scope_id, created_at, id);

alter table mcp_upstream_connection_secrets
  add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table mcp_upstream_connection_secrets alter column scope_id set not null;

alter table mcp_upstream_tool_sources
  add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table mcp_upstream_tool_sources alter column scope_id set not null;
create index if not exists mcp_upstream_tool_sources_scope_created_id_idx
  on mcp_upstream_tool_sources (scope_id, created_at, id);

create index if not exists workflow_extension_triggers_scope_created_id_idx
  on workflow_extension_triggers (scope_id, created_at, id);
