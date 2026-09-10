-- Keep first-seen IDs even if extensions disappear, so a restart cannot restore revoked grants.
create table console_permission_catalog_sync (
    singleton boolean primary key check (singleton),
    initialized boolean not null default false
);
insert into console_permission_catalog_sync (singleton) values (true);

create table console_permission_catalog_groups (
    group_kind text not null check (group_kind in ('settings_feature', 'other')),
    group_id text not null,
    first_seen_at timestamptz not null default now(),
    primary key (group_kind, group_id)
);

create table console_permission_catalog_operations (
    operation_id text primary key,
    first_seen_at timestamptz not null default now()
);
