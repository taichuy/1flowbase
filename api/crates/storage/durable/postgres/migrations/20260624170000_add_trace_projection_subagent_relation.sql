alter table application_run_trace_nodes
    add column source_flow_run_id uuid references flow_runs(id) on delete set null,
    add column source_trace_node_id uuid,
    add column parent_callback_task_id uuid,
    add column parent_tool_call_id text,
    add column trace_relation_kind text;

create index application_run_trace_nodes_source_flow_run_idx
    on application_run_trace_nodes (source_flow_run_id)
    where source_flow_run_id is not null;

create index application_run_trace_nodes_trace_relation_idx
    on application_run_trace_nodes (flow_run_id, trace_relation_kind)
    where trace_relation_kind is not null;
