alter table model_provider_request_logs
    add column if not exists is_retry boolean not null default false,
    add column if not exists retry_reason text;
