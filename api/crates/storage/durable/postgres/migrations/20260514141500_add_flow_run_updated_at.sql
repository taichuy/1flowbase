alter table flow_runs
    add column updated_at timestamptz;

update flow_runs
set updated_at = created_at
where updated_at is null;

alter table flow_runs
    alter column updated_at set not null,
    alter column updated_at set default now();
