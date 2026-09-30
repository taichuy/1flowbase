-- Physical rows are immutable model-call owners. Reserved contiguous cursor
-- slots preserve the existing Native paging contract without persisted children.
alter table runtime_events add column reserved_sequence_count bigint generated always as (
    case when payload ? '_context_occurrences'
        then jsonb_array_length(payload->'_context_occurrences'->'entries')::bigint else 0 end
) stored not null;
alter table runtime_events add constraint native_context_occurrence_version check (
    not (payload ? '_context_occurrences') or
    (event_type='provider_semantic_step' and node_run_id is not null
     and payload->>'source'='ai_native' and payload->>'kind'='model_call'
     and payload->'_context_occurrences'->>'version'='1'
     and jsonb_typeof(payload->'_context_occurrences'->'entries')='array')
);
create or replace function maintain_runtime_event_sequence_high_water() returns trigger language plpgsql as $$
begin
    update flow_runs f set runtime_event_sequence_high_water=greatest(f.runtime_event_sequence_high_water,b.last_sequence)
    from (select flow_run_id,max(sequence+reserved_sequence_count) last_sequence
          from appended_runtime_events group by flow_run_id) b where f.id=b.flow_run_id;
    return null;
end $$;

-- The old projection keeps the first cursor/ID and latest metadata/body when
-- an invocation retries the same step. Apply that same rule across both layouts.
create view provider_semantic_trajectory_read_steps as
with candidates as (
    select s.event_id,s.flow_run_id,s.node_run_id,s.invocation_id,s.provider_attempt_index,
           s.step_key,s.event_sequence,s.metadata-'_context_occurrences' metadata,
           s.created_at,s.body_event_id,s.snapshot_count,e.sequence source_sequence
    from provider_semantic_trajectory_steps s
    join runtime_events e on e.id=s.body_event_id
    union all
    select (entry.value->>'event_id')::uuid,e.flow_run_id,e.node_run_id,
           e.payload->>'invocation_id',(e.payload->>'provider_attempt_index')::bigint,
           entry.value->'metadata'->>'step_key',e.sequence+entry.ordinality,
           (e.payload-'_context_occurrences'-'_observation_body_ref'-'body_format'-'body') || (entry.value->'metadata'),
           e.created_at,e.id,1::bigint,e.sequence+entry.ordinality
    from runtime_events e
    cross join lateral jsonb_array_elements(e.payload->'_context_occurrences'->'entries') with ordinality entry(value,ordinality)
    where e.payload->'_context_occurrences'->>'version'='1'
), ranked as (
    select candidates.*,
           row_number() over identity_cursor ordinal,
           first_value(metadata) over identity_latest latest_metadata,
           first_value(body_event_id) over identity_latest latest_body_event_id,
           sum(snapshot_count) over identity_group total_snapshot_count
    from candidates
    window identity_group as (partition by flow_run_id,node_run_id,invocation_id,provider_attempt_index,step_key),
           identity_cursor as (identity_group order by event_sequence,event_id),
           identity_latest as (identity_group order by source_sequence desc,event_id desc)
)
select event_id,flow_run_id,node_run_id,invocation_id,provider_attempt_index,step_key,event_sequence,
       latest_metadata metadata,created_at,latest_body_event_id body_event_id,total_snapshot_count snapshot_count
from ranked where ordinal=1;
