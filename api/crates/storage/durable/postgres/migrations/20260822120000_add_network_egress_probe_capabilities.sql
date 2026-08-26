alter table network_egress_pool_members
    add column probe_http_status text not null default 'not_tested'
        check (probe_http_status in ('not_tested', 'succeeded', 'failed')),
    add column probe_https_status text not null default 'not_tested'
        check (probe_https_status in ('not_tested', 'succeeded', 'failed')),
    add column probe_exit_region text null;
