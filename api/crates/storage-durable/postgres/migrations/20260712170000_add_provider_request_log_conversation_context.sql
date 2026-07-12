alter table model_provider_request_logs
    add column if not exists application_id uuid,
    add column if not exists conversation_id text;
