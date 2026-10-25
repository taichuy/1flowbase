insert into application_run_trace_nodes as stored (
    id, scope_id, trace_node_id, flow_run_id, parent_trace_node_id,
    stable_locator, node_kind, owner_kind, owner_id, order_key, node_id,
    node_type, node_mode, node_alias, status, started_at, finished_at,
    duration_ms, metrics_payload, has_children, child_count, has_content,
    content_ref, source_flow_run_id, source_trace_node_id, parent_callback_task_id,
    parent_tool_call_id, trace_relation_kind, projection_version, source_watermark,
    raw_json_payloads
)
select id, scope_id, trace_node_id, flow_run_id, parent_trace_node_id,
    stable_locator, node_kind, owner_kind, owner_id, order_key, node_id,
    node_type, node_mode, node_alias, status, started_at, finished_at,
    duration_ms, metrics_packet::jsonb -> 0, has_children, child_count, has_content,
    content_ref_packet::jsonb ->> 0, source_flow_run_id, source_trace_node_id,
    parent_callback_task_id, parent_tool_call_id, trace_relation_kind,
    projection_version, source_watermark,
    jsonb_strip_nulls(jsonb_build_object(
        'metrics_payload', metrics_packet::jsonb -> 1,
        'content_ref', content_ref_packet::jsonb -> 1
    ))
from (
