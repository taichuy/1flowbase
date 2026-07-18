with versioned_documents as (
  select
    id,
    case
      when jsonb_typeof(document_payload) = 'object'
        and jsonb_typeof(document_payload -> 'blocks') = 'array'
      then jsonb_set(
        document_payload,
        '{blocks}',
        coalesce(
          (
            select jsonb_agg(
              case
                when jsonb_typeof(entry.block) = 'object'
                  and not (entry.block ? 'renderer_version')
                  then entry.block || jsonb_build_object('renderer_version', 'v1')
                else entry.block
              end
              order by entry.ordinality
            )
            from jsonb_array_elements(document_payload -> 'blocks')
              with ordinality as entry(block, ordinality)
          ),
          '[]'::jsonb
        ),
        true
      )
      else document_payload
    end as document_payload
  from frontstage_page_schemas
)
update frontstage_page_schemas schemas
set document_payload = versioned.document_payload,
    schema_payload = case
      when jsonb_typeof(schemas.schema_payload) = 'object'
        and jsonb_typeof(schemas.schema_payload -> 'blocks') = 'array'
        and jsonb_typeof(versioned.document_payload -> 'blocks') = 'array'
      then jsonb_set(
        schemas.schema_payload,
        '{blocks}',
        versioned.document_payload -> 'blocks',
        true
      )
      else schemas.schema_payload
    end,
    root_payload = case
      when jsonb_typeof(schemas.root_payload) = 'object'
        and jsonb_typeof(schemas.root_payload -> 'blocks') = 'array'
        and jsonb_typeof(versioned.document_payload -> 'blocks') = 'array'
      then jsonb_set(
        schemas.root_payload,
        '{blocks}',
        versioned.document_payload -> 'blocks',
        true
      )
      else schemas.root_payload
    end
from versioned_documents versioned
where schemas.id = versioned.id
  and jsonb_typeof(versioned.document_payload -> 'blocks') = 'array';
