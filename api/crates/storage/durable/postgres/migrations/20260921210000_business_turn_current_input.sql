-- Imported request history is retained evidence, not the current task input.
create or replace function application_run_log_task_user_input(run_id uuid) returns text
language sql stable as $$
    select coalesce(
        (
            select m.query from application_run_conversation_message_items m
            where m.flow_run_id=run_id and m.is_current and m.query is not null
            order by m.display_sequence desc limit 1
        ),
        (
            select m.content from application_run_conversation_message_items m
            where m.flow_run_id=run_id and m.is_current and m.role='user'
            order by m.display_sequence limit 1
        ),
        case
            when jsonb_typeof(f.log_context #> '{prompt,content}') = 'string'
            then nullif(btrim(f.log_context #>> '{prompt,content}'), '')
        end,
        case
            when jsonb_typeof(f.log_context #> '{prompt,content}') = 'array'
            then nullif(btrim((
                select string_agg(
                    coalesce(part->>'text', part->>'content', ''), '' order by ordinality)
                from jsonb_array_elements(f.log_context #> '{prompt,content}')
                    with ordinality element(part, ordinality)
            )), '')
        end
    )
    from flow_runs f
    where f.id = run_id
$$;

update application_run_log_tasks set user_input=application_run_log_task_user_input(id);
