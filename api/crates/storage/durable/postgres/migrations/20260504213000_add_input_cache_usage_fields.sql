alter table runtime_usage_ledger
    add column input_cache_hit_tokens bigint,
    add column input_cache_miss_tokens bigint;
