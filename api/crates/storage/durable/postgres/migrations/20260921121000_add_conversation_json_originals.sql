-- Lossless originals for conversation history and derived trace payloads.
alter table application_conversation_messages add column raw_json_payloads jsonb not null default '{}'::jsonb;
alter table application_run_conversation_message_items add column raw_json_payloads jsonb not null default '{}'::jsonb;
alter table application_run_trace_nodes add column raw_json_payloads jsonb not null default '{}'::jsonb;
alter table application_run_trace_node_contents add column raw_json_payloads jsonb not null default '{}'::jsonb;
