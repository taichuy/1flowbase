update plugin_tasks
set status = 'queued'
where status = 'pending';

update plugin_tasks
set status = 'succeeded'
where status = 'success';

alter table plugin_installations
    rename column install_path to installed_path;

alter table plugin_installations
    add column desired_state text not null default 'disabled',
    add column artifact_status text not null default 'missing',
    add column runtime_status text not null default 'inactive',
    add column availability_status text not null default 'disabled',
    add column package_path text,
    add column manifest_fingerprint text,
    add column last_load_error text;

update plugin_installations
set desired_state = case when enabled then 'active_requested' else 'disabled' end,
    artifact_status = case when enabled then 'install_incomplete' else 'missing' end,
    runtime_status = 'inactive',
    availability_status = case when enabled then 'install_incomplete' else 'disabled' end,
    package_path = null,
    manifest_fingerprint = null,
    last_load_error = null;

alter table plugin_installations
    drop column enabled;

alter table plugin_installations
    add constraint plugin_installations_desired_state_check
        check (desired_state in ('disabled', 'pending_restart', 'active_requested')),
    add constraint plugin_installations_artifact_status_check
        check (artifact_status in ('missing', 'staged', 'ready', 'corrupted', 'install_incomplete')),
    add constraint plugin_installations_runtime_status_check
        check (runtime_status in ('inactive', 'active', 'load_failed')),
    add constraint plugin_installations_availability_status_check
        check (availability_status in ('disabled', 'pending_restart', 'artifact_missing', 'install_incomplete', 'load_failed', 'available'));

alter table plugin_tasks
    drop constraint plugin_tasks_status_check;

alter table plugin_tasks
    add constraint plugin_tasks_status_check
        check (status in ('queued', 'running', 'succeeded', 'failed', 'canceled', 'timed_out'));
