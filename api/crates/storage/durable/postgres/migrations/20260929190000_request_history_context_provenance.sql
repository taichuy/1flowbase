-- v6 removes execution-derived prompts from request history. Context occupies
-- a separate sequence range: preserve all actual messages, their IDs and cursors.
delete from application_run_conversation_message_items
where context_source in ('application_config', 'effective_prompt');

-- The remaining v5 facts use the same derivation in v6. Promote their revision
-- without changing the retained-fact watermark or repairing anything on GET.
update application_run_conversation_message_items
set projection_version = 6,
    source_revision = regexp_replace(source_revision, '^v5:', 'v6:'),
    native_message = case
        when native_message->>'_log_source_revision' like 'v5:%'
        then jsonb_set(native_message, '{_log_source_revision}',
            to_jsonb(regexp_replace(native_message->>'_log_source_revision', '^v5:', 'v6:')))
        else native_message
    end,
    raw_json_payloads = case
        when raw_json_payloads ? 'native_message'
        -- The native writer appends this marker last. Match that final field
        -- only: parsing NUL-bearing JSON or replacing a nested provider field
        -- would corrupt original evidence. Preserve every other byte.
        then jsonb_set(raw_json_payloads, '{native_message}',
            to_jsonb(regexp_replace(raw_json_payloads->>'native_message',
                '("_log_source_revision"[[:space:]]*:[[:space:]]*")v5:([^"]*"[[:space:]]*})$',
                '\1v6:\2')))
        else raw_json_payloads
    end
where projection_version = 5;
