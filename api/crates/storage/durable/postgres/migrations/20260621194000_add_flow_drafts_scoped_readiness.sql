alter table flow_drafts add column if not exists scope_id uuid;
alter table flow_drafts add column if not exists created_at timestamptz;
alter table flow_drafts add column if not exists created_by uuid;
update flow_drafts drafts
   set scope_id = flows.scope_id,
       created_at = coalesce(drafts.created_at, flows.created_at),
       created_by = coalesce(drafts.created_by, flows.created_by)
  from flows
 where drafts.flow_id = flows.id
   and (
       drafts.scope_id is null
       or drafts.created_at is null
       or drafts.created_by is null
   );
alter table flow_drafts alter column scope_id set not null;
alter table flow_drafts alter column created_at set default now();
alter table flow_drafts alter column created_at set not null;
alter table flow_drafts alter column created_by set not null;
create index if not exists flow_drafts_scope_created_id_idx
    on flow_drafts (scope_id, created_at, id);
