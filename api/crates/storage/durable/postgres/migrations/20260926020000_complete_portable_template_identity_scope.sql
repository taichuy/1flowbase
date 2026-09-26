alter table portable_template_identities
    add column id uuid,
    add column scope_id uuid,
    add column updated_at timestamptz;

update portable_template_identities
set id = gen_random_uuid(),
    scope_id = workspace_id,
    updated_at = created_at;

alter table portable_template_identities
    alter column id set not null,
    alter column scope_id set not null,
    alter column updated_at set default now(),
    alter column updated_at set not null,
    add constraint portable_template_identities_scope_matches_workspace
        check (scope_id = workspace_id);

create unique index portable_template_identities_id_uidx
    on portable_template_identities (id);

create index portable_template_identities_scope_created_id_idx
    on portable_template_identities (scope_id, created_at, id);
