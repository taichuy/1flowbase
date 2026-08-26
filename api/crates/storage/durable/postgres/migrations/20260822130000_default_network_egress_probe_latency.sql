update network_egress_pool_members
set probe_latency_ms = 0
where probe_latency_ms is null;

alter table network_egress_pool_members
    alter column probe_latency_ms set default 0,
    alter column probe_latency_ms set not null;
