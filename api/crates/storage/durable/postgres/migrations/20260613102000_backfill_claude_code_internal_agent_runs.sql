create temporary table claude_code_internal_agent_result_backfill on commit drop as
with agent_results as (
    select
        parent_runs.id as parent_flow_run_id,
        parent_tasks.id as parent_callback_task_id,
        tool_call ->> 'id' as tool_call_id,
        btrim(tool_call #>> '{arguments,prompt}') as prompt,
        btrim(tool_result ->> 'content') as content,
        coalesce(parent_tasks.completed_at, parent_runs.finished_at, now()) as completed_at,
        parent_runs.application_id,
        parent_runs.api_key_id,
        parent_runs.external_user,
        parent_runs.external_conversation_id
    from flow_run_callback_tasks parent_tasks
    join flow_runs parent_runs
      on parent_runs.id = parent_tasks.flow_run_id
    cross join lateral jsonb_array_elements(
        case
            when jsonb_typeof(parent_tasks.request_payload -> 'tool_calls') = 'array'
            then parent_tasks.request_payload -> 'tool_calls'
            else '[]'::jsonb
        end
    ) as tool_call
    join lateral jsonb_array_elements(
        case
            when jsonb_typeof(parent_tasks.response_payload -> 'tool_results') = 'array'
            then parent_tasks.response_payload -> 'tool_results'
            else '[]'::jsonb
        end
    ) as tool_result
      on tool_result ->> 'tool_call_id' = tool_call ->> 'id'
    where parent_runs.compatibility_mode = 'anthropic-messages-v1'
      and parent_runs.status = 'succeeded'
      and parent_runs.external_user is not null
      and parent_runs.external_conversation_id is not null
      and parent_tasks.callback_kind = 'llm_tool_calls'
      and parent_tasks.status = 'completed'
      and tool_call ->> 'name' = 'Agent'
      and nullif(btrim(tool_call #>> '{arguments,prompt}'), '') is not null
      and nullif(btrim(tool_result ->> 'content'), '') is not null
      and btrim(tool_result ->> 'content') not like '<tool_use_error>%'
),
matching_subagents as (
    select distinct on (sub_runs.id)
        sub_runs.id as sub_flow_run_id,
        agent_results.parent_flow_run_id,
        agent_results.parent_callback_task_id,
        agent_results.tool_call_id,
        agent_results.content,
        agent_results.completed_at
    from agent_results
    join flow_runs sub_runs
      on sub_runs.application_id = agent_results.application_id
     and sub_runs.api_key_id is not distinct from agent_results.api_key_id
     and sub_runs.external_user = agent_results.external_user
     and sub_runs.external_conversation_id = agent_results.external_conversation_id
     and sub_runs.id <> agent_results.parent_flow_run_id
    where sub_runs.compatibility_mode = 'anthropic-messages-v1'
      and sub_runs.status = 'waiting_callback'
      and coalesce(
            sub_runs.input_payload #>> '{node-start,query}',
            sub_runs.input_payload #>> '{start,query}',
            sub_runs.input_payload #>> '{query}',
            ''
          ) = agent_results.prompt
      and (
            position(
                'cc_is_subagent=true'
                in coalesce(
                    sub_runs.input_payload #>> '{node-start,system}',
                    sub_runs.input_payload #>> '{start,system}',
                    sub_runs.input_payload #>> '{system}',
                    ''
                )
            ) > 0
            or (
                position(
                    'Agent threads always have their cwd reset between bash calls'
                    in coalesce(
                        sub_runs.input_payload #>> '{node-start,system}',
                        sub_runs.input_payload #>> '{start,system}',
                        sub_runs.input_payload #>> '{system}',
                        ''
                    )
                ) > 0
                and position(
                    'the parent agent reads your text output'
                    in coalesce(
                        sub_runs.input_payload #>> '{node-start,system}',
                        sub_runs.input_payload #>> '{start,system}',
                        sub_runs.input_payload #>> '{system}',
                        ''
                    )
                ) > 0
            )
          )
    order by sub_runs.id, agent_results.completed_at desc, agent_results.parent_callback_task_id desc
)
select * from matching_subagents;

update flow_runs
set status = 'succeeded',
    output_payload = jsonb_build_object(
        'answer', backfill.content,
        'compatibility', jsonb_build_object(
            'claude_code_internal_agent_result', true,
            'parent_flow_run_id', backfill.parent_flow_run_id::text,
            'parent_callback_task_id', backfill.parent_callback_task_id::text,
            'tool_call_id', backfill.tool_call_id
        )
    ),
    error_payload = null,
    finished_at = backfill.completed_at,
    updated_at = backfill.completed_at
from claude_code_internal_agent_result_backfill backfill
where flow_runs.id = backfill.sub_flow_run_id
  and flow_runs.status = 'waiting_callback';

update node_runs
set status = 'succeeded',
    output_payload = jsonb_build_object(
        'answer', backfill.content,
        'compatibility', jsonb_build_object(
            'claude_code_internal_agent_result', true,
            'parent_flow_run_id', backfill.parent_flow_run_id::text,
            'parent_callback_task_id', backfill.parent_callback_task_id::text,
            'tool_call_id', backfill.tool_call_id
        )
    ),
    error_payload = null,
    finished_at = backfill.completed_at
from claude_code_internal_agent_result_backfill backfill
where node_runs.flow_run_id = backfill.sub_flow_run_id
  and node_runs.status = 'waiting_callback';

update flow_run_callback_tasks
set status = 'cancelled',
    completed_at = coalesce(flow_run_callback_tasks.completed_at, backfill.completed_at)
from claude_code_internal_agent_result_backfill backfill
where flow_run_callback_tasks.flow_run_id = backfill.sub_flow_run_id
  and flow_run_callback_tasks.status = 'pending';

update flow_run_callback_resume_attempts
set status = 'cancelled',
    completed_at = coalesce(flow_run_callback_resume_attempts.completed_at, backfill.completed_at),
    updated_at = backfill.completed_at
from claude_code_internal_agent_result_backfill backfill
where flow_run_callback_resume_attempts.flow_run_id = backfill.sub_flow_run_id
  and flow_run_callback_resume_attempts.status in ('received', 'processing');
