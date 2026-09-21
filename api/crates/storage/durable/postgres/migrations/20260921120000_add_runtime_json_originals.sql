-- Adapter-private originals. Existing rows need no data rewrite.
-- JSONB business columns remain query projections; serialized JSON text keeps
-- U+0000 and object keys reversible without introducing user-visible markers.
alter table flow_runs add column raw_json_payloads jsonb not null default '{}'::jsonb;
alter table node_runs add column raw_json_payloads jsonb not null default '{}'::jsonb;
alter table flow_run_callback_resume_attempts add column raw_json_payloads jsonb not null default '{}'::jsonb;
alter table flow_run_callback_tasks add column raw_json_payloads jsonb not null default '{}'::jsonb;
alter table flow_run_checkpoints add column raw_json_payloads jsonb not null default '{}'::jsonb;
alter table flow_run_resume_claims add column raw_json_payloads jsonb not null default '{}'::jsonb;
alter table flow_run_events add column raw_json_payloads jsonb not null default '{}'::jsonb;
alter table runtime_events add column raw_json_payloads jsonb not null default '{}'::jsonb;
alter table runtime_canonical_contents add column raw_json_payloads jsonb not null default '{}'::jsonb;
alter table flow_run_tool_callback_inbox add column raw_json_payloads jsonb not null default '{}'::jsonb;

-- Returns PostgreSQL JSON (not JSONB): JSON preserves the escape in its wire
-- representation, and serde_json restores it only after it leaves PostgreSQL.
create function runtime_original_json(projection jsonb, originals jsonb, field_name text)
returns json language sql immutable parallel safe as $$
    select case when originals ? field_name
        then (originals ->> field_name)::json
        else projection::json end
$$;
