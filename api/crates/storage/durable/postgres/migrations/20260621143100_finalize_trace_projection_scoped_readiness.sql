alter table application_run_trace_projection_statuses
    alter column id set not null;

alter table application_run_trace_projection_statuses
    alter column scope_id set not null;

create index if not exists application_run_trace_projection_statuses_scope_created_id_idx
    on application_run_trace_projection_statuses (scope_id, created_at, id);

create index if not exists application_run_trace_projection_statuses_scope_status_updated_flow_run_idx
    on application_run_trace_projection_statuses (scope_id, status, updated_at, flow_run_id);

alter table application_run_trace_nodes
    alter column id set not null;

alter table application_run_trace_nodes
    alter column scope_id set not null;

create index if not exists application_run_trace_nodes_scope_created_id_idx
    on application_run_trace_nodes (scope_id, created_at, id);

create index if not exists application_run_trace_nodes_scope_flow_parent_order_idx
    on application_run_trace_nodes (scope_id, flow_run_id, parent_trace_node_id, order_key, trace_node_id);

create index if not exists application_run_trace_nodes_scope_flow_owner_idx
    on application_run_trace_nodes (scope_id, flow_run_id, owner_kind, owner_id);

alter table application_run_trace_node_contents
    alter column id set not null;

alter table application_run_trace_node_contents
    alter column flow_run_id set not null;

alter table application_run_trace_node_contents
    alter column scope_id set not null;

create index if not exists application_run_trace_node_contents_scope_created_id_idx
    on application_run_trace_node_contents (scope_id, created_at, id);

create index if not exists application_run_trace_node_contents_scope_flow_created_trace_idx
    on application_run_trace_node_contents (scope_id, flow_run_id, created_at, trace_node_id);
