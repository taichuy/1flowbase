-- Unbounded Text values can exceed PostgreSQL's B-tree entry limit. Ordered Tree
-- keeps automatic prefix indexes only for bounded String/Enum fields; explicit
-- Text searches retain the same semantics and may use a sequential scan.
--
-- This follows the repository's single-file convention for irreversible
-- migrations. Recreating these indexes in a down migration would restore the
-- deterministic write failure this migration removes.
do $$
declare
    field record;
begin
    for field in
        select fields.id
        from model_fields fields
        join model_definitions definitions
          on definitions.id = fields.data_model_id
        where definitions.template_provider = 'core'
          and definitions.template_code = 'ordered_tree'
          and definitions.template_version = 'v1'
          and definitions.source_kind = 'main_source'
          and fields.field_kind = 'text'
          and not fields.is_system
    loop
        execute format(
            'drop index if exists %I.%I',
            current_schema(),
            'idx_ot_prefix_' || replace(field.id::text, '-', '')
        );
    end loop;
end
$$;
