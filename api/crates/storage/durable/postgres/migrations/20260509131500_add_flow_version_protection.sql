alter table flow_versions
    add column summary_is_custom boolean not null default false,
    add column is_protected boolean not null default false;

create index flow_versions_flow_protected_sequence_idx
    on flow_versions (flow_id, is_protected desc, sequence asc);
