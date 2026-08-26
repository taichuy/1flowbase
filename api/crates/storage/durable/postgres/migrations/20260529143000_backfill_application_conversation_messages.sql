create or replace function pg_temp.application_run_log_uuid_from_md5(input_text text)
returns uuid
language sql
immutable
as $$
    select (
        substr(hash, 1, 8) || '-' ||
        substr(hash, 9, 4) || '-' ||
        substr(hash, 13, 4) || '-' ||
        substr(hash, 17, 4) || '-' ||
        substr(hash, 21, 12)
    )::uuid
    from (select md5(input_text) as hash) hashed;
$$;

create or replace function pg_temp.application_run_log_preview_text(input_text text)
returns text
language plpgsql
stable
as $$
declare
    trimmed text;
    parsed jsonb;
begin
    trimmed := nullif(btrim(input_text), '');
    if trimmed is null then
        return null;
    end if;

    begin
        parsed := trimmed::jsonb;
        if jsonb_typeof(parsed) = 'string' then
            return nullif(btrim(parsed #>> '{}'), '');
        end if;
    exception when others then
    end;

    return trimmed;
end;
$$;

create or replace function pg_temp.application_run_log_text(input_json jsonb)
returns text
language sql
stable
as $$
    select case
        when input_json is null then null
        when jsonb_typeof(input_json) = 'string' then nullif(btrim(input_json #>> '{}'), '')
        when jsonb_typeof(input_json) = 'object' then coalesce(
            pg_temp.application_run_log_preview_text(input_json ->> 'preview'),
            case
                when jsonb_typeof(input_json -> 'text') = 'string'
                then nullif(btrim(input_json #>> '{text}'), '')
            end,
            case
                when jsonb_typeof(input_json -> 'content') = 'string'
                then nullif(btrim(input_json #>> '{content}'), '')
            end
        )
        else null
    end;
$$;

create or replace function pg_temp.application_run_log_first_text(input_json jsonb, keys text[])
returns text
language sql
stable
as $$
    select pg_temp.application_run_log_text(input_json -> candidate.key)
    from unnest(keys) with ordinality as candidate(key, key_order)
    where pg_temp.application_run_log_text(input_json -> candidate.key) is not null
    order by candidate.key_order asc
    limit 1;
$$;

create or replace function pg_temp.application_run_log_start_payload(input_json jsonb)
returns jsonb
language sql
stable
as $$
    select coalesce(input_json -> 'node-start', input_json -> 'start', input_json, '{}'::jsonb);
$$;

create or replace function pg_temp.application_run_log_answer_text(input_json jsonb)
returns text
language sql
stable
as $$
    select coalesce(
        pg_temp.application_run_log_first_text(
            input_json,
            array['answer', 'text', 'output', 'content', 'message']
        ),
        pg_temp.application_run_log_text(input_json #> '{error,message}')
    );
$$;

create or replace function pg_temp.application_run_log_message_content(input_json jsonb)
returns text
language sql
stable
as $$
    select coalesce(
        pg_temp.application_run_log_text(input_json -> 'content'),
        (
            select nullif(string_agg(parts.part_text, '' order by parts.part_order), '')
            from (
                select
                    pg_temp.application_run_log_text(part.value) as part_text,
                    part.part_order
                from jsonb_array_elements(
                    case
                        when jsonb_typeof(input_json -> 'content') = 'array'
                        then input_json -> 'content'
                        else '[]'::jsonb
                    end
                ) with ordinality as part(value, part_order)
            ) parts
            where parts.part_text is not null
        )
    );
$$;

create temporary table application_run_log_message_projection_backfill
on commit drop
as
with candidate_runs as (
    select
        flow_runs.id as flow_run_id,
        applications.workspace_id as scope_id,
        flow_runs.application_id,
        api_keys.id as api_key_id,
        flow_runs.external_user,
        coalesce(
            flow_runs.external_conversation_id,
            'flow-run:' || flow_runs.id::text
        ) as conversation_key,
        flow_runs.status,
        flow_runs.started_at,
        flow_runs.finished_at,
        flow_runs.created_at,
        flow_runs.updated_at,
        flow_runs.input_payload,
        flow_runs.output_payload,
        flow_runs.error_payload,
        pg_temp.application_run_log_start_payload(flow_runs.input_payload) as start_payload
    from flow_runs
    join applications on applications.id = flow_runs.application_id
    left join api_keys on api_keys.id = flow_runs.api_key_id
    where flow_runs.run_mode = 'published_api_run'
      and flow_runs.status in ('succeeded', 'failed', 'cancelled')
),
run_texts as (
    select
        candidate_runs.*,
        coalesce(
            pg_temp.application_run_log_text(start_payload -> 'system'),
            pg_temp.application_run_log_text(input_payload -> 'system')
        ) as system_text,
        coalesce(
            pg_temp.application_run_log_first_text(
                input_payload,
                array['query', 'question', 'prompt', 'message', 'input', 'input_text']
            ),
            pg_temp.application_run_log_first_text(
                start_payload,
                array['query', 'question', 'prompt', 'message', 'input', 'input_text']
            )
        ) as user_text,
        coalesce(
            pg_temp.application_run_log_answer_text(output_payload),
            pg_temp.application_run_log_answer_text(error_payload)
        ) as answer_text
    from candidate_runs
),
history_candidates as (
    select
        run_texts.flow_run_id,
        item.item_order,
        case item.value ->> 'role'
            when 'system' then 'system'
            when 'user' then 'user'
            when 'assistant' then 'assistant'
        end as role,
        pg_temp.application_run_log_message_content(item.value) as content
    from run_texts
    cross join lateral jsonb_array_elements(
        case
            when jsonb_typeof(coalesce(start_payload -> 'history', start_payload -> 'messages')) = 'array'
            then coalesce(start_payload -> 'history', start_payload -> 'messages')
            else '[]'::jsonb
        end
    ) with ordinality as item(value, item_order)
),
history_messages as (
    select
        history_candidates.flow_run_id,
        history_candidates.role,
        history_candidates.content,
        row_number() over (
            partition by history_candidates.flow_run_id
            order by history_candidates.item_order asc
        )::bigint as history_ordinal
    from history_candidates
    where history_candidates.role is not null
      and history_candidates.content is not null
),
history_counts as (
    select
        history_messages.flow_run_id,
        count(*)::bigint as history_count
    from history_messages
    group by history_messages.flow_run_id
),
message_candidates as (
    select
        run_texts.flow_run_id,
        run_texts.scope_id,
        run_texts.application_id,
        run_texts.api_key_id,
        run_texts.external_user,
        run_texts.conversation_key,
        run_texts.status,
        run_texts.started_at,
        run_texts.finished_at,
        run_texts.created_at,
        run_texts.updated_at,
        'system'::text as role,
        run_texts.system_text as content,
        1::bigint as ordinal
    from run_texts
    where run_texts.system_text is not null

    union all

    select
        run_texts.flow_run_id,
        run_texts.scope_id,
        run_texts.application_id,
        run_texts.api_key_id,
        run_texts.external_user,
        run_texts.conversation_key,
        run_texts.status,
        run_texts.started_at,
        run_texts.finished_at,
        run_texts.created_at,
        run_texts.updated_at,
        history_messages.role,
        history_messages.content,
        (
            case when run_texts.system_text is not null then 1 else 0 end
            + history_messages.history_ordinal
        )::bigint as ordinal
    from run_texts
    join history_messages on history_messages.flow_run_id = run_texts.flow_run_id

    union all

    select
        run_texts.flow_run_id,
        run_texts.scope_id,
        run_texts.application_id,
        run_texts.api_key_id,
        run_texts.external_user,
        run_texts.conversation_key,
        run_texts.status,
        run_texts.started_at,
        run_texts.finished_at,
        run_texts.created_at,
        run_texts.updated_at,
        'user'::text as role,
        run_texts.user_text as content,
        (
            case when run_texts.system_text is not null then 1 else 0 end
            + coalesce(history_counts.history_count, 0)
            + 1
        )::bigint as ordinal
    from run_texts
    left join history_counts on history_counts.flow_run_id = run_texts.flow_run_id
    where run_texts.user_text is not null

    union all

    select
        run_texts.flow_run_id,
        run_texts.scope_id,
        run_texts.application_id,
        run_texts.api_key_id,
        run_texts.external_user,
        run_texts.conversation_key,
        run_texts.status,
        run_texts.started_at,
        run_texts.finished_at,
        run_texts.created_at,
        run_texts.updated_at,
        'assistant'::text as role,
        run_texts.answer_text as content,
        (
            case when run_texts.system_text is not null then 1 else 0 end
            + coalesce(history_counts.history_count, 0)
            + case when run_texts.user_text is not null then 1 else 0 end
            + 1
        )::bigint as ordinal
    from run_texts
    left join history_counts on history_counts.flow_run_id = run_texts.flow_run_id
    where run_texts.answer_text is not null
)
select
    message_candidates.flow_run_id,
    message_candidates.scope_id,
    message_candidates.application_id,
    message_candidates.api_key_id,
    message_candidates.external_user,
    message_candidates.conversation_key,
    message_candidates.role,
    message_candidates.content,
    (
        extract(epoch from message_candidates.started_at)::bigint * 1000000
        + message_candidates.ordinal
    )::bigint as sequence,
    message_candidates.status,
    message_candidates.started_at,
    message_candidates.finished_at,
    message_candidates.created_at,
    message_candidates.updated_at
from message_candidates
where message_candidates.content is not null;

create temporary table application_run_log_conversation_projection_backfill
on commit drop
as
select
    projection.scope_id,
    projection.application_id,
    projection.api_key_id,
    projection.external_user,
    projection.conversation_key,
    (array_agg(projection.content order by projection.started_at asc, projection.sequence asc)
        filter (where projection.role = 'user'))[1] as title,
    min(projection.started_at) as created_at,
    max(projection.updated_at) as updated_at
from application_run_log_message_projection_backfill projection
group by
    projection.scope_id,
    projection.application_id,
    projection.api_key_id,
    projection.external_user,
    projection.conversation_key;

insert into application_conversations (
    id,
    scope_id,
    application_id,
    api_key_id,
    external_user,
    external_conversation_id,
    title,
    created_at,
    updated_at
)
select
    pg_temp.application_run_log_uuid_from_md5(
        'application-conversation:' ||
        projection.application_id::text || ':' ||
        coalesce(projection.api_key_id::text, '') || ':' ||
        coalesce(projection.external_user, '') || ':' ||
        projection.conversation_key
    ),
    projection.scope_id,
    projection.application_id,
    projection.api_key_id,
    projection.external_user,
    projection.conversation_key,
    projection.title,
    projection.created_at,
    projection.updated_at
from application_run_log_conversation_projection_backfill projection
where not exists (
    select 1
    from application_conversations existing
    where existing.application_id = projection.application_id
      and existing.external_conversation_id = projection.conversation_key
      and existing.api_key_id is not distinct from projection.api_key_id
      and existing.external_user is not distinct from projection.external_user
);

update application_conversations conversations
set title = coalesce(nullif(conversations.title, ''), projection.title),
    updated_at = greatest(conversations.updated_at, projection.updated_at)
from application_run_log_conversation_projection_backfill projection
where conversations.application_id = projection.application_id
  and conversations.external_conversation_id = projection.conversation_key
  and conversations.api_key_id is not distinct from projection.api_key_id
  and conversations.external_user is not distinct from projection.external_user
  and projection.title is not null;

insert into application_conversation_messages (
    id,
    scope_id,
    conversation_id,
    application_id,
    flow_run_id,
    node_run_id,
    role,
    content,
    sequence,
    status,
    started_at,
    finished_at,
    created_at,
    updated_at
)
select
    pg_temp.application_run_log_uuid_from_md5(
        'application-conversation-message:' ||
        projection.flow_run_id::text || ':' ||
        projection.sequence::text || ':' ||
        projection.role
    ),
    projection.scope_id,
    conversation.id,
    projection.application_id,
    projection.flow_run_id,
    null,
    projection.role,
    projection.content,
    projection.sequence,
    projection.status,
    projection.started_at,
    projection.finished_at,
    projection.started_at,
    projection.updated_at
from application_run_log_message_projection_backfill projection
join lateral (
    select conversations.id
    from application_conversations conversations
    where conversations.application_id = projection.application_id
      and conversations.external_conversation_id = projection.conversation_key
      and conversations.api_key_id is not distinct from projection.api_key_id
      and conversations.external_user is not distinct from projection.external_user
    order by conversations.updated_at desc, conversations.id desc
    limit 1
) conversation on true
on conflict (conversation_id, flow_run_id, sequence) do update
set role = excluded.role,
    content = excluded.content,
    status = excluded.status,
    started_at = excluded.started_at,
    finished_at = excluded.finished_at,
    updated_at = excluded.updated_at;
