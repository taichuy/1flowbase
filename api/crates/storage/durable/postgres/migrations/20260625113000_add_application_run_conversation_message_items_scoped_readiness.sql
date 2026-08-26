create index if not exists application_run_conversation_message_items_scope_created_id_idx
    on application_run_conversation_message_items (scope_id, created_at, id);
