) as incoming (
    id, flow_run_id, scope_id, trace_node_id, content_kind,
    payload_packet, source_refs_packet
)
where true
on conflict (trace_node_id) do update set
    scope_id = excluded.scope_id,
    content_kind = excluded.content_kind,
    payload = excluded.payload,
    source_refs = excluded.source_refs,
    raw_json_payloads = excluded.raw_json_payloads,
    created_at = excluded.created_at,
    updated_at = excluded.updated_at
where stored.flow_run_id = excluded.flow_run_id
    and row(stored.scope_id, stored.content_kind, stored.payload, stored.source_refs, stored.raw_json_payloads)
        is distinct from
        row(excluded.scope_id, excluded.content_kind, excluded.payload, excluded.source_refs, excluded.raw_json_payloads)
