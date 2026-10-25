insert into application_run_trace_node_contents as stored (
    id, flow_run_id, scope_id, trace_node_id, content_kind,
    payload, source_refs, raw_json_payloads
)
select id, flow_run_id, scope_id, trace_node_id, content_kind,
    payload_packet::jsonb -> 0, source_refs_packet::jsonb -> 0,
    jsonb_strip_nulls(jsonb_build_object(
        'payload', payload_packet::jsonb -> 1,
        'source_refs', source_refs_packet::jsonb -> 1
    ))
from (
