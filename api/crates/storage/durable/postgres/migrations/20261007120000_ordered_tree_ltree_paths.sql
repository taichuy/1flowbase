-- Keep the extension outside isolated application/test schemas. Concurrent first
-- installers (including test databases) must agree on the same transaction lock.
select pg_advisory_xact_lock(hashtextextended('1flowbase:extension:ltree', 0));
create extension if not exists ltree with schema public;

-- parent_id remains the sole relationship source. Paths are private storage
-- projections: UUID labels survive rename and sibling-rank changes.
create function ordered_tree_maintain_path() returns trigger language plpgsql as $$
declare
    parent_path public.ltree;
    label public.ltree;
begin
    if tg_when = 'AFTER' then
        if new.tree_path is distinct from old.tree_path then
            execute format(
                'update %I.%I set tree_path = $1 OPERATOR(public.||) public.subpath(tree_path, public.nlevel($2)) where scope_id = $3 and tree_partition_id = $4 and ARRAY[tree_path] OPERATOR(public.<@) $2 and id <> $5',
                tg_table_schema, tg_table_name
            ) using new.tree_path, old.tree_path, new.scope_id, new.tree_partition_id, new.id;
        end if;
        return null;
    end if;

    -- Only the descendant rewrite issued by this trigger can write a derived
    -- path directly. It must never change relationships or identity.
    if tg_op = 'UPDATE' and pg_trigger_depth() > 1
       and new.parent_id is not distinct from old.parent_id
       and new.scope_id = old.scope_id and new.tree_partition_id = old.tree_partition_id
       and new.id = old.id then
        return new;
    end if;

    if tg_op = 'DELETE' then
        perform pg_advisory_xact_lock(hashtextextended(tg_argv[0] || ':' || old.scope_id::text || ':' || old.tree_partition_id::text, 0));
        return old;
    end if;
    perform pg_advisory_xact_lock(hashtextextended(tg_argv[0] || ':' || new.scope_id::text || ':' || new.tree_partition_id::text, 0));
    if tg_op = 'UPDATE' then
        if new.id <> old.id or new.scope_id <> old.scope_id or new.tree_partition_id <> old.tree_partition_id then
            raise exception 'ordered-tree identity and partition are immutable' using errcode = '23514';
        end if;
        if new.tree_path is distinct from old.tree_path then
            raise exception 'ordered-tree tree_path is derived' using errcode = '23514';
        end if;
        if new.parent_id is not distinct from old.parent_id then
            return new;
        end if;
    elsif new.tree_path is not null then
        raise exception 'ordered-tree tree_path is derived' using errcode = '23514';
    end if;

    label := replace(new.id::text, '-', '')::public.ltree;
    if new.parent_id is null then
        new.tree_path := label;
    else
        execute format('select tree_path from %I.%I where scope_id = $1 and tree_partition_id = $2 and id = $3', tg_table_schema, tg_table_name)
            into parent_path using new.scope_id, new.tree_partition_id, new.parent_id;
        if parent_path is null then
            raise exception 'ordered-tree parent missing in scope/partition' using errcode = '23503';
        end if;
        if new.parent_id = new.id or (tg_op = 'UPDATE' and parent_path OPERATOR(public.<@) old.tree_path) then
            raise exception 'ordered-tree cycle' using errcode = '23514';
        end if;
        new.tree_path := parent_path OPERATOR(public.||) label;
    end if;
    return new;
end $$;

-- Shared by migrations and future dynamic-tree creation. Never adopt a user
-- column named tree_path: a collision or an unreachable row aborts the DDL.
create function ordered_tree_install_path(target regclass, model_id uuid) returns void language plpgsql as $$
declare
    target_schema text;
    target_table text;
    unresolved bigint;
begin
    select n.nspname, c.relname into strict target_schema, target_table
      from pg_class c join pg_namespace n on n.oid = c.relnamespace where c.oid = target;
    execute format('lock table %I.%I in access exclusive mode', target_schema, target_table);
    execute format('alter table %I.%I add column tree_path public.ltree', target_schema, target_table);
    execute format($backfill$
        with recursive paths(id, scope_id, tree_partition_id, tree_path) as (
            select id, scope_id, tree_partition_id, replace(id::text, '-', '')::public.ltree
            from %1$I.%2$I where parent_id is null
            union all
            select child.id, child.scope_id, child.tree_partition_id,
                   paths.tree_path OPERATOR(public.||) replace(child.id::text, '-', '')::public.ltree
            from %1$I.%2$I child join paths on child.parent_id = paths.id
                and child.scope_id = paths.scope_id and child.tree_partition_id = paths.tree_partition_id
        )
        update %1$I.%2$I node set tree_path = paths.tree_path from paths
        where node.id = paths.id and node.scope_id = paths.scope_id and node.tree_partition_id = paths.tree_partition_id
    $backfill$, target_schema, target_table);
    execute format('select count(*) from %I.%I where tree_path is null', target_schema, target_table) into unresolved;
    if unresolved <> 0 then
        raise exception 'ordered-tree path backfill failed for %: % unreachable rows (cycle or missing scoped parent)', target, unresolved using errcode = '23514';
    end if;
    execute format('alter table %I.%I alter column tree_path set not null', target_schema, target_table);
    -- Scalar ltree GiST leaves retain the full path and can hit the page
    -- limit on deep UUID trees. The one-element array opclass stores only a
    -- fixed-size lossy signature; PostgreSQL rechecks the exact path predicate.
    execute format('create index %I on %I.%I using gist ((ARRAY[tree_path]) public.gist__ltree_ops)', 'idx_ot_path_' || replace(model_id::text, '-', ''), target_schema, target_table);
    execute format('create trigger ordered_tree_path_before before insert or update or delete on %I.%I for each row execute function %I.ordered_tree_maintain_path(%L)', target_schema, target_table, current_schema(), model_id::text);
    execute format('create trigger ordered_tree_path_after after update of parent_id on %I.%I for each row execute function %I.ordered_tree_maintain_path(%L)', target_schema, target_table, current_schema(), model_id::text);
end $$;

do $$
declare model record;
begin
    if exists(
        select 1 from model_fields f join model_definitions d on d.id = f.data_model_id
        where d.template_provider = 'core' and d.template_code = 'ordered_tree' and d.template_version = 'v1'
          and (f.code = 'tree_path' or f.physical_column_name = 'tree_path')
    ) then
        raise exception 'ordered-tree reserved tree_path metadata conflict' using errcode = '23514';
    end if;
    for model in
        select id, physical_table_name from model_definitions
        where template_provider = 'core' and template_code = 'ordered_tree'
          and template_version = 'v1' and source_kind = 'main_source'
          and physical_table_name not in ('departments', 'frontstage_block_nodes')
    loop
        if exists(select 1 from model_fields where data_model_id = model.id and (code = 'tree_path' or physical_column_name = 'tree_path')) then
            raise exception 'ordered-tree reserved tree_path field conflict for %', model.id using errcode = '23514';
        end if;
        perform ordered_tree_install_path(format('%I.%I', current_schema(), model.physical_table_name)::regclass, model.id);
    end loop;
    perform ordered_tree_install_path('departments'::regclass, 'de9a0000-0000-4000-8000-000000000001'::uuid);
    perform ordered_tree_install_path('frontstage_block_nodes'::regclass, 'e6aa0cc5-dfc0-8d8d-b6c8-b9bd0113a61a'::uuid);
end $$;
