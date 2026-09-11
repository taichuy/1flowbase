            with tasks as (
                select coalesce(log_task_run_id,flow_run_id) as anchor_id,
                    application_id,scope_id,api_key_id,coalesce(external_user,'') as external_user_key,count(*)::bigint as invocation_count,
                    min(started_at) as started_at,min(created_at) as created_at,
                    max(updated_at) as updated_at,
                    case when bool_and(finished_at is not null) then max(finished_at) end as finished_at,
                    (array_agg(status order by case status
                        when 'running' then 0 when 'waiting_callback' then 1
                        when 'waiting_human' then 2 when 'paused' then 3
                        when 'queued' then 4 when 'failed' then 5
                        when 'cancelled' then 6 when 'incomplete' then 7
                        when 'succeeded' then 8 else 9 end,
                        created_at desc,flow_run_id))[1] as status,
                    sum(total_tokens)::bigint as total_tokens,
                    sum(input_tokens)::bigint as input_tokens,
                    sum(output_tokens)::bigint as output_tokens,
                    sum(input_cache_hit_tokens)::bigint as input_cache_hit_tokens,
                    sum(unique_node_count)::bigint as unique_node_count,
                    sum(tool_callback_count)::bigint as tool_callback_count
                from application_run_log_summaries where /* log_scope */
                group by application_id,scope_id,api_key_id,coalesce(external_user,''),coalesce(log_task_run_id,flow_run_id)
            )
            select merged.*
            from tasks t join application_run_log_summaries s on s.flow_run_id=t.anchor_id
                and s.application_id=t.application_id and s.scope_id=t.scope_id
                and s.api_key_id is not distinct from t.api_key_id
                and coalesce(s.external_user,'')=t.external_user_key
            cross join lateral jsonb_populate_record(s,
                (to_jsonb(t)-'anchor_id'-'external_user_key') || jsonb_build_object(
                    'input_payload','{}'::jsonb,
                    'input_cache_hit_rate',case when coalesce(t.input_tokens,0)+t.input_cache_hit_tokens>0
                        then t.input_cache_hit_tokens::double precision /
                            (coalesce(t.input_tokens,0)+t.input_cache_hit_tokens)::double precision end
                )) merged
