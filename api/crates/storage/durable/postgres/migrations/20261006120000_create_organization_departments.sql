create table departments (
 id uuid primary key,
 scope_id uuid not null references workspaces(id) on delete cascade,
 tree_partition_id uuid not null,
 parent_id uuid,
 sibling_rank text collate "C" not null,
 name text not null check (length(btrim(name)) > 0),
 created_at timestamptz not null default now(), updated_at timestamptz not null default now(),
 created_by uuid, updated_by uuid,
 unique(scope_id,id), unique(scope_id,tree_partition_id,id),
 check(tree_partition_id = scope_id), check(parent_id is null or parent_id <> id),
 foreign key(scope_id,tree_partition_id,parent_id) references departments(scope_id,tree_partition_id,id) on delete restrict
);
create unique index departments_sibling_rank on departments(scope_id,tree_partition_id,parent_id,sibling_rank) where parent_id is not null;
create unique index departments_root_rank on departments(scope_id,tree_partition_id,sibling_rank) where parent_id is null;
create index departments_parent on departments(scope_id,parent_id);
create table department_role_bindings (
 id uuid primary key, scope_id uuid not null, department_id uuid not null,
 role_id uuid not null references roles(id) on delete cascade,
 unique(department_id,role_id),
 foreign key(scope_id,department_id) references departments(scope_id,id) on delete cascade
);
create table user_department_bindings (
 id uuid primary key, scope_id uuid not null, user_id uuid not null, department_id uuid not null,
 is_primary boolean not null default false,
 unique(scope_id,user_id,department_id),
 foreign key(scope_id,user_id) references workspace_memberships(workspace_id,user_id) on delete cascade,
 foreign key(scope_id,department_id) references departments(scope_id,id) on delete restrict
);
create unique index user_departments_primary on user_department_bindings(scope_id,user_id) where is_primary;
create index user_departments_department on user_department_bindings(scope_id,department_id,user_id);
-- Deferred check permits transactional replacement, while retaining the primary invariant
-- for every durable writer (including native SQL).
create function organization_check_primary() returns trigger language plpgsql as $$
declare s uuid; u uuid;
begin
 if tg_op='UPDATE' and (new.scope_id<>old.scope_id or new.user_id<>old.user_id) then
   if exists(select 1 from user_department_bindings where scope_id=old.scope_id and user_id=old.user_id)
     and (select count(*) from user_department_bindings where scope_id=old.scope_id and user_id=old.user_id and is_primary) <> 1 then
     raise exception 'organization primary department required' using errcode='23514';
   end if;
 end if;
 s := coalesce(new.scope_id,old.scope_id); u := coalesce(new.user_id,old.user_id);
 if exists(select 1 from user_department_bindings where scope_id=s and user_id=u)
    and (select count(*) from user_department_bindings where scope_id=s and user_id=u and is_primary) <> 1 then
    raise exception 'organization primary department required' using errcode='23514';
 end if;
 return null;
end $$;
create constraint trigger organization_primary after insert or update or delete on user_department_bindings
 deferrable initially deferred for each row execute function organization_check_primary();
create function organization_check_role_scope() returns trigger language plpgsql as $$
begin
 if not exists(select 1 from roles where id=new.role_id and scope_kind='workspace'
   and workspace_id=new.scope_id and scope_id=new.scope_id and code<>'root' and system_kind is null) then
   raise exception 'organization role scope invalid' using errcode='23514';
 end if;
 return new;
end $$;
create trigger organization_role_scope before insert or update on department_role_bindings
 for each row execute function organization_check_role_scope();
