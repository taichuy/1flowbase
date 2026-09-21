with observations as (
    select node_run_id,metadata->>'invocation_id' as invocation_id,
        (metadata->>'provider_attempt_index')::bigint as attempt,count(*) as observed
    from provider_protocol_trajectory_events
    where flow_run_id=$1 and ($2::uuid is null or node_run_id=$2) and event_type='provider_protocol_observation'
    group by 1,2,3
), latest_protocol as (
    select distinct on (node_run_id,metadata->>'invocation_id',metadata->>'provider_attempt_index')
        node_run_id,metadata->>'invocation_id' as invocation_id,
        (metadata->>'provider_attempt_index')::bigint as attempt,metadata
    from provider_protocol_trajectory_events
    where flow_run_id=$1 and ($2::uuid is null or node_run_id=$2) and event_type='provider_protocol_integrity'
    order by node_run_id,metadata->>'invocation_id',metadata->>'provider_attempt_index',event_sequence desc
), protocol as (
    select coalesce(o.node_run_id,i.node_run_id) as node_run_id,coalesce(o.invocation_id,i.invocation_id) as invocation_id,
        coalesce(o.attempt,i.attempt) as attempt,coalesce(o.observed,0) as observed,
        coalesce((i.metadata->>'persist_failed_count')::bigint,0) as failed,
        coalesce(i.metadata->>'status'='complete'
            and (i.metadata->>'observed_count')::bigint=coalesce(o.observed,0)
            and coalesce((i.metadata->>'persist_failed_count')::bigint,0)=0
            and coalesce((i.metadata->>'dropped_count')::bigint,0)=0,false) as complete,
        coalesce(i.metadata->>'status'='unavailable' and coalesce(o.observed,0)=0
            and coalesce((i.metadata->>'persist_failed_count')::bigint,0)=0
            and coalesce((i.metadata->>'dropped_count')::bigint,0)=0,false) as unavailable
    from observations o full join latest_protocol i using(node_run_id,invocation_id,attempt)
), steps as (
    select node_run_id,invocation_id,provider_attempt_index as attempt,
        coalesce(metadata->>'source','supplier_protocol') as source,
        sum(snapshot_count) as observed,
        bool_or(metadata->>'status' in ('incomplete','unavailable','pending')) as incomplete
    from provider_semantic_trajectory_steps where flow_run_id=$1 and ($2::uuid is null or node_run_id=$2)
    group by 1,2,3,4
), native as (
    select coalesce(s.node_run_id,i.node_run_id) as node_run_id,coalesce(s.invocation_id,i.invocation_id) as invocation_id,
        coalesce(s.attempt,i.provider_attempt_index) as attempt,
        coalesce((i.metadata->>'persist_failed_count')::bigint,0) as failed,
        coalesce(i.metadata->>'status'='complete'
            and (i.metadata->>'observed_count')::bigint=coalesce(s.observed,0)
            and coalesce((i.metadata->>'persist_failed_count')::bigint,0)=0
            and coalesce((i.metadata->>'dropped_count')::bigint,0)=0
            and not coalesce(s.incomplete,false),false) as complete
    from (select * from steps where source='ai_native') s
    full join (select * from native_trajectory_integrity where flow_run_id=$1 and ($2::uuid is null or node_run_id=$2)) i
        on s.node_run_id=i.node_run_id and s.invocation_id=i.invocation_id and s.attempt=i.provider_attempt_index
), semantic as (
    select failed,complete from native
    union all
    select coalesce(p.failed,0),coalesce(p.complete,false) and not coalesce(s.incomplete,false)
    from (select * from steps where source<>'ai_native') s
    left join protocol p using(node_run_id,invocation_id,attempt)
    union all
    -- Raw-only historical attempts cannot disappear in a mixed node history.
    select p.failed,false from protocol p
    where not p.unavailable
        and not exists(select 1 from steps s where s.node_run_id=p.node_run_id and s.invocation_id=p.invocation_id and s.attempt=p.attempt)
        and not exists(select 1 from native n where n.node_run_id=p.node_run_id and n.invocation_id=p.invocation_id and n.attempt=p.attempt)
)
select (select coalesce(sum(observed),0)::bigint from protocol) as observation_count,
    (select coalesce(sum(failed),0)::bigint from protocol) as protocol_persist_failed_count,
    (select case when count(*)=0 or bool_and(unavailable) then 'not_recorded'
        when bool_and(complete) then 'complete' else 'incomplete' end from protocol) as protocol_integrity,
    (select coalesce(sum(failed),0)::bigint from semantic) as persist_failed_count,
    (select case when count(*)=0 then 'not_recorded'
        when bool_and(complete) then 'complete' else 'incomplete' end from semantic) as integrity
