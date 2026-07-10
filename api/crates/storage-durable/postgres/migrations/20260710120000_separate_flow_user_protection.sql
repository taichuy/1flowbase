alter table flow_versions
    rename column is_protected to is_user_protected;

drop index if exists flow_versions_flow_protected_sequence_idx;

create index flow_versions_flow_user_protected_sequence_idx
    on flow_versions (flow_id, is_user_protected desc, sequence asc);
