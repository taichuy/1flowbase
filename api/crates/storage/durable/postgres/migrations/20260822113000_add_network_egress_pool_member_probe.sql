alter table network_egress_pool_members
    add column probe_status text not null default 'not_tested'
        check (probe_status in ('not_tested', 'succeeded', 'failed')),
    add column probe_latency_ms integer null check (probe_latency_ms >= 0),
    add column probe_exit_ip text null,
    add column probe_error_code text null,
    add column last_probed_at timestamptz null;
