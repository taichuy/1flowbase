-- Native items remain the trace facts. This function only selects the final
-- business answer; an error payload or a tool call is never a business answer.
create or replace function application_run_log_task_final_output(members uuid[])
returns table(run_id uuid, content text)
language sql stable as $$
    select m.flow_run_id, coalesce(m.content, m.answer)
    from application_run_conversation_message_items m
    join flow_runs f on f.id=m.flow_run_id
    join unnest($1) with ordinality member(run_id, position) on member.run_id=m.flow_run_id
    where nullif(btrim(coalesce(m.content,m.answer)), '') is not null
      and (
        (m.role='assistant' and m.native_message #>> '{_source_item,type}'='message'
            and m.native_message #>> '{_source_item,phase}'='final_answer'
            and (
                (jsonb_typeof(m.native_message #> '{_source_item,content}')='string'
                    and nullif(btrim(m.native_message #>> '{_source_item,content}'),'') is not null)
                or exists (
                    select 1 from jsonb_array_elements(case
                        when jsonb_typeof(m.native_message #> '{_source_item,content}')='array'
                        then m.native_message #> '{_source_item,content}' else '[]'::jsonb end) block
                    where block->>'type' in ('text','output_text')
                      and nullif(btrim(block->>'text'),'') is not null
                )
            ))
        or (f.status='succeeded' and m.output_source='persisted_answer')
        or (f.status='succeeded' and m.role='assistant'
            and m.native_message #>> '{_source_item,type}'='message'
            and m.native_message #>> '{_source_item,role}'='assistant'
            and m.native_message #>> '{_source_item,phase}' is null
            and jsonb_typeof(m.native_message #> '{_source_item,content}')='array'
            and exists (
                select 1 from jsonb_array_elements(case when jsonb_typeof(m.native_message #> '{_source_item,content}')='array' then m.native_message #> '{_source_item,content}' else '[]'::jsonb end) block
                where block->>'type' in ('text','output_text')
                  and nullif(btrim(block->>'text'),'') is not null
            )
            and not exists (
                select 1 from application_run_conversation_message_items later
                where later.flow_run_id=m.flow_run_id
                  and later.output_source='provider_output_item'
                  and later.display_sequence > m.display_sequence
            ))
      )
    order by member.position desc,m.display_sequence desc
    limit 1
$$;

-- Historical retained projections are reused; GET does not repair old rows.
select application_run_log_task_refresh(id) from application_run_log_tasks;
