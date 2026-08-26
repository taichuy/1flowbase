alter table audit_logs add column if not exists scope_id uuid;
alter table audit_logs add column if not exists created_by uuid;
alter table audit_logs add column if not exists updated_by uuid;
alter table audit_logs add column if not exists updated_at timestamptz;
update audit_logs
   set scope_id = coalesce(workspace_id, '00000000-0000-0000-0000-000000000000'::uuid),
       created_by = coalesce(created_by, actor_user_id),
       updated_by = coalesce(updated_by, actor_user_id),
       updated_at = coalesce(updated_at, created_at)
 where scope_id is null
    or updated_at is null
    or (created_by is null and actor_user_id is not null)
    or (updated_by is null and actor_user_id is not null);
alter table audit_logs alter column scope_id set not null;
alter table audit_logs alter column updated_at set default now();
alter table audit_logs alter column updated_at set not null;
create index if not exists audit_logs_scope_created_id_idx
    on audit_logs (scope_id, created_at, id);

create or replace function set_audit_log_scope_id()
returns trigger
language plpgsql
as $$
begin
  if new.workspace_id is null then
    new.scope_id := '00000000-0000-0000-0000-000000000000'::uuid;
  else
    new.scope_id := new.workspace_id;
  end if;
  if new.updated_at is null then
    new.updated_at := new.created_at;
  end if;
  if new.created_by is null then
    new.created_by := new.actor_user_id;
  end if;
  if new.updated_by is null then
    new.updated_by := new.actor_user_id;
  end if;
  return new;
end;
$$;

drop trigger if exists audit_logs_set_scope_id on audit_logs;
create trigger audit_logs_set_scope_id
before insert or update of workspace_id, scope_id, actor_user_id, updated_at on audit_logs
for each row execute function set_audit_log_scope_id();
