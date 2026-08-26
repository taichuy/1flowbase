alter table api_keys
    add column if not exists last_used_at timestamptz null;
