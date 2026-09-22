, events as (
 select 'native:'||s.event_id::text event_id,s.event_id source_id,s.event_sequence,
 'provider_semantic_step'::text event_type,s.created_at,
 case when s.metadata->>'kind' in ('tool_call','tool_result','tool_response') then 'tools' else 'requests' end category,
 s.flow_run_id,s.node_run_id,s.metadata->>'status' status,
 left(coalesce(s.metadata->>'preview',s.metadata->>'kind','provider_semantic_step'),240) preview,
 s.metadata || jsonb_build_object('source',coalesce(s.metadata->>'source','supplier_protocol'),'flow_run_id',s.flow_run_id,'node_run_id',s.node_run_id,'run_mode',r.run_mode,'purpose',coalesce(s.metadata->>'purpose','unknown')) metadata
 from scope_runs r join provider_semantic_trajectory_steps s on s.flow_run_id=r.id
 join node_runs n on n.id=s.node_run_id and n.flow_run_id=s.flow_run_id
 union all
 select 'workflow:'||e.id,e.id,e.sequence,e.event_type,e.created_at,
 case when e.event_type in ('public_run_callback_cancelled','tool_callback_completed','tool_callback_failed') then 'tools' else 'requests' end,
 e.flow_run_id,e.node_run_id,e.payload->>'status',e.event_type,null::jsonb
 from scope_runs r join flow_run_events e on e.flow_run_id=r.id
 where e.event_type in ('flow_run_started','flow_run_execution_started','flow_run_resumed','flow_run_succeeded','flow_run_failed','flow_run_cancelled','flow_run_completed','flow_run_paused','flow_run_waiting_callback','flow_run_waiting_human','public_run_callback_cancelled','tool_callback_completed','tool_callback_failed','data_model_side_effect_confirmed')
 and (e.node_run_id is null or exists(select 1 from node_runs n where n.id=e.node_run_id and n.flow_run_id=e.flow_run_id))
 union all
 select 'runtime:'||e.id,e.id,e.sequence,e.event_type,e.created_at,
 case when e.event_type in ('assistant_tool_call_started','assistant_tool_call_finished') then 'tools' else 'requests' end,
 e.flow_run_id,e.node_run_id,e.payload->>'status',e.event_type,null::jsonb
 from scope_runs r join runtime_events e on e.flow_run_id=r.id
 where e.event_type in ('flow_finished','flow_failed','flow_cancelled','flow_incomplete','waiting_callback','waiting_human','assistant_tool_call_started','assistant_tool_call_finished')
 and (e.node_run_id is null or exists(select 1 from node_runs n where n.id=e.node_run_id and n.flow_run_id=e.flow_run_id))
 union all
 select 'node_started:'||n.id,n.id,null::bigint,'node_started',n.started_at,'nodes',n.flow_run_id,n.id,'running',n.node_alias,null::jsonb
 from scope_runs r join node_runs n on n.flow_run_id=r.id
 union all
 select 'node_finished:'||n.id,n.id,null::bigint,'node_finished',n.finished_at,'nodes',n.flow_run_id,n.id,n.status,n.node_alias,null::jsonb
 from scope_runs r join node_runs n on n.flow_run_id=r.id where n.finished_at is not null
), enriched as (
 select e.*,case split_part(e.event_id,':',1) when 'node_started' then 0 when 'workflow' then 1 when 'native' then 2 when 'runtime' then 2 else 4 end source_rank,r.task_run_id,r.parent_task_run_id,r.depth,r.is_round,n.node_id,n.node_alias,n.node_type
 from events e join scope_runs r on r.id=e.flow_run_id
 left join node_runs n on n.id=e.node_run_id and n.flow_run_id=e.flow_run_id
)
