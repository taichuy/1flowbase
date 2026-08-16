-- Frontstage blocks were draft-only before Block Node Descriptor v1 became the
-- sole source of truth. Keep Page/Tab and non-block document metadata, while
-- removing both legacy document blocks and draft node/code rows.
delete from frontstage_block_nodes;
delete from frontstage_block_codes;

update frontstage_page_schemas
set document_payload = document_payload - 'blocks',
    schema_payload = schema_payload - 'blocks',
    root_payload = root_payload - 'blocks',
    updated_at = now()
where document_payload ? 'blocks'
   or schema_payload ? 'blocks'
   or root_payload ? 'blocks';

alter table frontstage_page_schemas
  add constraint frontstage_page_schemas_document_without_blocks_check
    check (not (document_payload ? 'blocks')),
  add constraint frontstage_page_schemas_schema_without_blocks_check
    check (not (schema_payload ? 'blocks')),
  add constraint frontstage_page_schemas_root_without_blocks_check
    check (not (root_payload ? 'blocks'));

alter table frontstage_block_nodes
  add constraint frontstage_block_nodes_descriptor_v1_check check (
    jsonb_typeof(runtime_descriptor -> 'catalog') = 'object'
    and jsonb_typeof(runtime_descriptor -> 'contribution') = 'object'
    and jsonb_typeof(runtime_descriptor -> 'props') = 'object'
    and jsonb_typeof(runtime_descriptor -> 'ports') = 'object'
    and jsonb_typeof(runtime_descriptor -> 'ports' -> 'inputs') = 'array'
    and jsonb_typeof(runtime_descriptor -> 'ports' -> 'outputs') = 'array'
    and jsonb_typeof(runtime_descriptor -> 'x-layout') = 'object'
    and jsonb_typeof(runtime_descriptor -> 'x-presentation') = 'object'
    and jsonb_typeof(runtime_descriptor -> 'runtime') = 'object'
    and nullif(btrim(runtime_descriptor ->> 'id'), '') is not null
    and nullif(btrim(runtime_descriptor ->> 'codeRef'), '') is not null
    and nullif(btrim(runtime_descriptor ->> 'rendererVersion'), '') is not null
  );
