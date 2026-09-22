with page as (
    select s.* from provider_semantic_trajectory_steps s
    where s.flow_run_id=$1 and ($2::uuid is null or s.node_run_id=$2)
      and s.event_sequence>$3 and ($5::text is null or s.metadata->>'trigger_request_id'=$5)
    order by s.event_sequence limit $4
)
select s.event_id,s.event_sequence,'provider_semantic_step' as event_type,s.created_at,
    s.metadata || jsonb_build_object('source',coalesce(s.metadata->>'source','supplier_protocol'),
        'flow_run_id',s.flow_run_id,'node_run_id',s.node_run_id,'run_mode',r.run_mode,
        'purpose',coalesce(s.metadata->>'purpose','unknown')) as metadata,
    coalesce(links.items,'[]'::jsonb) as links
from page s join flow_runs r on r.id=s.flow_run_id
left join lateral (
    select jsonb_agg(to_jsonb(l) order by l.relation,l.request_id) as items from (
        select 'trigger' as relation,c.flow_run_id,c.request_id,null::text as response_id
        from client_trajectory_captures c
        where c.flow_run_id=s.flow_run_id and c.request_id=(s.metadata->>'trigger_request_id')::uuid
        union
        select 'context',c.flow_run_id,c.request_id,c.response_id
        from client_trajectory_captures c join flow_runs parent on parent.id=c.flow_run_id
        where parent.application_id=r.application_id
          and c.flow_run_id=(s.metadata->>'context_flow_run_id')::uuid
          and c.response_id=s.metadata->>'context_response_id'
        union
        select distinct 'context',c.flow_run_id,c.request_id,s.metadata->>'context_response_id'
        from client_trajectory_steps c join flow_runs parent on parent.id=c.flow_run_id
        where parent.application_id=r.application_id
          and c.flow_run_id=(s.metadata->>'context_flow_run_id')::uuid
          and c.metadata->>'response_id'=s.metadata->>'context_response_id'
          and c.metadata->>'origin'='emitted'
    ) l
) links on true
order by s.event_sequence
