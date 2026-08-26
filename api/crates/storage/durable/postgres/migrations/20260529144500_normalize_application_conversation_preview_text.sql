create or replace function pg_temp.application_run_log_preview_text(input_text text)
returns text
language plpgsql
stable
as $$
declare
    trimmed text;
    parsed jsonb;
    candidate text;
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

    if left(trimmed, 1) = '"' then
        candidate := trimmed;
        if right(candidate, 1) <> '"' then
            candidate := candidate || '"';
        end if;

        begin
            parsed := candidate::jsonb;
            if jsonb_typeof(parsed) = 'string' then
                return nullif(btrim(parsed #>> '{}'), '');
            end if;
        exception when others then
        end;

        candidate := btrim(trimmed, '"');
        candidate := replace(candidate, E'\\n', E'\n');
        candidate := replace(candidate, E'\\r', E'\r');
        candidate := replace(candidate, E'\\t', E'\t');
        candidate := replace(candidate, E'\\"', '"');
        return nullif(btrim(candidate), '');
    end if;

    return trimmed;
end;
$$;

with assistant_previews as (
    select
        messages.id,
        pg_temp.application_run_log_preview_text(
            flow_runs.output_payload #>> '{answer,preview}'
        ) as content,
        flow_runs.updated_at
    from application_conversation_messages messages
    join flow_runs on flow_runs.id = messages.flow_run_id
    where messages.role = 'assistant'
      and messages.content like '"%'
      and flow_runs.output_payload #>> '{answer,preview}' is not null
)
update application_conversation_messages messages
set content = assistant_previews.content,
    updated_at = greatest(messages.updated_at, assistant_previews.updated_at)
from assistant_previews
where messages.id = assistant_previews.id
  and assistant_previews.content is not null
  and messages.content is distinct from assistant_previews.content;
