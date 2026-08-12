-- Ordered-tree scope remains the authorization boundary. A separate structural
-- partition lets one scoped model host multiple independent trees.
do $$
declare
    model record;
    model_suffix text;
begin
    for model in
        select id, physical_table_name
        from model_definitions
        where template_provider = 'core'
          and template_code = 'ordered_tree'
          and template_version = 'v1'
          and source_kind = 'main_source'
    loop
        model_suffix := replace(model.id::text, '-', '');

        if exists (
            select 1
            from model_fields fields
            where fields.data_model_id = model.id
              and (
                  fields.code = 'tree_partition_id'
                  or fields.physical_column_name = 'tree_partition_id'
              )
              and not (
                  fields.code = 'tree_partition_id'
                  and fields.physical_column_name = 'tree_partition_id'
                  and fields.is_system
              )
        ) then
            raise exception
                'ordered-tree model % reserves tree_partition_id but existing user metadata owns that name',
                model.id;
        end if;

        execute format(
            'alter table %I add column if not exists tree_partition_id uuid',
            model.physical_table_name
        );
        execute format(
            'update %I set tree_partition_id = scope_id where tree_partition_id is null',
            model.physical_table_name
        );
        execute format(
            'alter table %I alter column tree_partition_id set not null',
            model.physical_table_name
        );

        execute format(
            'alter table %I drop constraint if exists %I',
            model.physical_table_name,
            'fk_ot_parent_' || model_suffix
        );
        execute format(
            'alter table %I drop constraint if exists %I',
            model.physical_table_name,
            'uq_ot_scope_id_' || model_suffix
        );
        execute format('drop index if exists %I', 'idx_ot_siblings_' || model_suffix);
        execute format('drop index if exists %I', 'uq_ot_sibling_' || model_suffix);
        execute format('drop index if exists %I', 'uq_ot_root_rank_' || model_suffix);

        execute format(
            'alter table %I add constraint %I unique (scope_id, tree_partition_id, id)',
            model.physical_table_name,
            'uq_ot_scope_id_' || model_suffix
        );
        execute format(
            'alter table %I add constraint %I foreign key (scope_id, tree_partition_id, parent_id) references %I (scope_id, tree_partition_id, id) on delete restrict',
            model.physical_table_name,
            'fk_ot_parent_' || model_suffix,
            model.physical_table_name
        );
        execute format(
            'create index %I on %I (scope_id, tree_partition_id, parent_id, sibling_rank, id)',
            'idx_ot_siblings_' || model_suffix,
            model.physical_table_name
        );
        execute format(
            'create unique index %I on %I (scope_id, tree_partition_id, parent_id, sibling_rank) where parent_id is not null',
            'uq_ot_sibling_' || model_suffix,
            model.physical_table_name
        );
        execute format(
            'create unique index %I on %I (scope_id, tree_partition_id, sibling_rank) where parent_id is null',
            'uq_ot_root_rank_' || model_suffix,
            model.physical_table_name
        );

        -- Existing metadata may be user-owned. Only fill the reserved system-field
        -- gap; never rewrite an existing row with the same code.
        insert into model_fields (
            id, data_model_id, code, title, physical_column_name, field_kind,
            is_system, is_writable, is_required, api_required, is_unique,
            display_options, relation_options, sort_order, availability_status,
            scope_id, created_by, updated_by
        )
        select
            gen_random_uuid(), model.id, 'tree_partition_id', 'tree_partition_id',
            'tree_partition_id', 'many_to_one', true, false, true, false, false,
            '{}'::jsonb, '{}'::jsonb, 8, 'available', definitions.scope_id, null, null
        from model_definitions definitions
        where definitions.id = model.id
          and not exists (
              select 1
              from model_fields fields
              where fields.data_model_id = model.id
                and fields.code = 'tree_partition_id'
          );
    end loop;
end
$$;
