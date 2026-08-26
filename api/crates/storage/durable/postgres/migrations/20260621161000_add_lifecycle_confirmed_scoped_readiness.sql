alter table application_publication_versions add column if not exists scope_id uuid;
alter table application_publication_versions add column if not exists updated_by uuid;
alter table application_publication_versions add column if not exists updated_at timestamptz;
update application_publication_versions
   set scope_id = applications.scope_id,
       updated_by = coalesce(application_publication_versions.updated_by, application_publication_versions.created_by),
       updated_at = coalesce(application_publication_versions.updated_at, application_publication_versions.created_at)
  from applications
 where application_publication_versions.application_id = applications.id
   and (
       application_publication_versions.scope_id is null
       or application_publication_versions.updated_at is null
   );
alter table application_publication_versions alter column scope_id set not null;
alter table application_publication_versions alter column updated_at set default now();
alter table application_publication_versions alter column updated_at set not null;
create index if not exists application_publication_versions_scope_created_id_idx
    on application_publication_versions (scope_id, created_at, id);

alter table flow_versions add column if not exists scope_id uuid;
alter table flow_versions add column if not exists updated_by uuid;
alter table flow_versions add column if not exists updated_at timestamptz;
update flow_versions
   set scope_id = flows.scope_id,
       updated_by = coalesce(flow_versions.updated_by, flow_versions.created_by),
       updated_at = coalesce(flow_versions.updated_at, flow_versions.created_at)
  from flows
 where flow_versions.flow_id = flows.id
   and (
       flow_versions.scope_id is null
       or flow_versions.updated_at is null
   );
alter table flow_versions alter column scope_id set not null;
alter table flow_versions alter column updated_at set default now();
alter table flow_versions alter column updated_at set not null;
create index if not exists flow_versions_scope_created_id_idx
    on flow_versions (scope_id, created_at, id);

alter table model_failover_queue_snapshots add column if not exists scope_id uuid;
alter table model_failover_queue_snapshots add column if not exists created_by uuid;
alter table model_failover_queue_snapshots add column if not exists updated_by uuid;
update model_failover_queue_snapshots snapshots
   set scope_id = templates.scope_id,
       created_by = coalesce(snapshots.created_by, templates.created_by),
       updated_by = coalesce(snapshots.updated_by, templates.created_by)
  from model_failover_queue_templates templates
 where snapshots.queue_template_id = templates.id
   and snapshots.scope_id is null;
alter table model_failover_queue_snapshots alter column scope_id set not null;
create index if not exists model_failover_queue_snapshots_scope_created_id_idx
    on model_failover_queue_snapshots (scope_id, created_at, id);
