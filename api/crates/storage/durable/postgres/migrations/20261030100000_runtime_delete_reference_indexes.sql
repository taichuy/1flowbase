-- Event deletion cascades to legacy trajectory steps. Without this index each
-- deleted event scans the entire steps table, including unrelated applications.
create index client_trajectory_step_event_owner
    on client_trajectory_steps (event_id) where event_id is not null;

-- Native trajectory bodies and node/span cascades follow independent FK
-- columns; flow-prefixed page indexes cannot serve these ownership lookups.
create index provider_semantic_trajectory_body_event_owner
    on provider_semantic_trajectory_steps (body_event_id) where body_event_id is not null;
create index runtime_events_node_owner
    on runtime_events (node_run_id) where node_run_id is not null;
create index runtime_events_span_owner
    on runtime_events (span_id) where span_id is not null;
create index runtime_events_parent_span_owner
    on runtime_events (parent_span_id) where parent_span_id is not null;
