-- Frontstage block nodes use the ordered-tree storage contract without becoming
-- user-visible model metadata. Existing tab documents remain as a migration
-- snapshot until the frontend cutover removes the legacy document projection.
do $$
begin
  if exists (
    select 1
    from frontstage_page_schemas schemas
    where jsonb_typeof(schemas.document_payload) <> 'object'
       or (
         schemas.document_payload ? 'blocks'
         and jsonb_typeof(schemas.document_payload -> 'blocks') <> 'array'
       )
       or (
         jsonb_typeof(schemas.document_payload -> 'child_containers') = 'array'
         and jsonb_array_length(schemas.document_payload -> 'child_containers') > 0
       )
       or exists (
         select 1
         from jsonb_array_elements(
           case when jsonb_typeof(schemas.document_payload -> 'blocks') = 'array'
             then schemas.document_payload -> 'blocks' else '[]'::jsonb end
         ) block
         where (
             jsonb_typeof(block -> 'block_ids') = 'array'
             and jsonb_array_length(block -> 'block_ids') > 0
           ) or (
             jsonb_typeof(block -> 'child_container_target_ids') = 'array'
             and jsonb_array_length(block -> 'child_container_target_ids') > 0
           ) or (
             jsonb_typeof(block -> 'childContainerTargetIds') = 'array'
             and jsonb_array_length(block -> 'childContainerTargetIds') > 0
           )
       )
  ) then
    raise exception 'frontstage block node migration rejected legacy child-container data';
  end if;

  if exists (
    select 1
    from frontstage_page_schemas schemas
    cross join lateral jsonb_array_elements(
      case when jsonb_typeof(schemas.document_payload -> 'blocks') = 'array'
        then schemas.document_payload -> 'blocks' else '[]'::jsonb end
    ) block
    where jsonb_typeof(block) <> 'object'
       or nullif(btrim(block ->> 'id'), '') is null
       or nullif(btrim(block ->> 'codeRef'), '') is null
  ) then
    raise exception 'frontstage block node migration rejected blocks without stable id or codeRef';
  end if;

  if exists (
    select 1
    from frontstage_page_schemas schemas
    cross join lateral jsonb_array_elements(
      case when jsonb_typeof(schemas.document_payload -> 'blocks') = 'array'
        then schemas.document_payload -> 'blocks' else '[]'::jsonb end
    ) block
    group by schemas.workspace_id, schemas.tab_id, block ->> 'id'
    having count(*) > 1
  ) or exists (
    select 1
    from frontstage_page_schemas schemas
    join frontstage_page_tabs tabs
      on tabs.workspace_id = schemas.workspace_id and tabs.id = schemas.tab_id
    cross join lateral jsonb_array_elements(
      case when jsonb_typeof(schemas.document_payload -> 'blocks') = 'array'
        then schemas.document_payload -> 'blocks' else '[]'::jsonb end
    ) block
    group by schemas.workspace_id, tabs.page_id, block ->> 'id'
    having count(*) > 1
  ) then
    raise exception 'frontstage block node migration rejected duplicate public block ids';
  end if;

  if exists (
    select 1
    from frontstage_page_schemas schemas
    join frontstage_page_tabs tabs
      on tabs.workspace_id = schemas.workspace_id and tabs.id = schemas.tab_id
    cross join lateral jsonb_array_elements(
      case when jsonb_typeof(schemas.document_payload -> 'blocks') = 'array'
        then schemas.document_payload -> 'blocks' else '[]'::jsonb end
    ) block
    left join frontstage_block_codes codes
      on codes.workspace_id = schemas.workspace_id
     and codes.page_id = tabs.page_id
     and codes.code_ref = block ->> 'codeRef'
    where codes.id is null
  ) then
    raise exception 'frontstage block node migration rejected block without source code';
  end if;
end $$;

create unique index if not exists frontstage_page_tabs_workspace_page_id_uidx
  on frontstage_page_tabs (workspace_id, page_id, id);

create table frontstage_block_nodes (
  id uuid primary key,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  created_by uuid,
  updated_by uuid,
  scope_id uuid not null,
  tree_partition_id uuid not null,
  parent_id uuid,
  sibling_rank text collate "C" not null,
  block_id text not null,
  tab_id uuid not null,
  presentation text not null,
  title text,
  code_ref text not null,
  schema_version bigint not null default 1,
  input_mapping jsonb not null default '{}'::jsonb,
  output_mapping jsonb not null default '{}'::jsonb,
  runtime_descriptor jsonb not null,
  constraint frontstage_block_nodes_scope_page_id_uidx unique (scope_id, tree_partition_id, id),
  constraint frontstage_block_nodes_scope_page_tab_id_uidx unique (scope_id, tree_partition_id, tab_id, id),
  constraint frontstage_block_nodes_public_id_uidx unique (scope_id, tree_partition_id, block_id),
  constraint frontstage_block_nodes_code_ref_uidx unique (scope_id, tree_partition_id, code_ref),
  constraint frontstage_block_nodes_parent_self_check check (parent_id is null or parent_id <> id),
  constraint frontstage_block_nodes_presentation_check check (presentation in ('page', 'drawer', 'modal', 'inline')),
  constraint frontstage_block_nodes_schema_version_check check (schema_version = 1),
  constraint frontstage_block_nodes_input_mapping_check check (jsonb_typeof(input_mapping) = 'object'),
  constraint frontstage_block_nodes_output_mapping_check check (jsonb_typeof(output_mapping) = 'object'),
  constraint frontstage_block_nodes_runtime_descriptor_check check (jsonb_typeof(runtime_descriptor) = 'object'),
  constraint frontstage_block_nodes_page_fkey foreign key (scope_id, tree_partition_id)
    references frontstage_pages (workspace_id, id) on delete cascade,
  constraint frontstage_block_nodes_tab_fkey foreign key (scope_id, tree_partition_id, tab_id)
    references frontstage_page_tabs (workspace_id, page_id, id) on delete cascade,
  constraint frontstage_block_nodes_parent_fkey foreign key (scope_id, tree_partition_id, parent_id)
    references frontstage_block_nodes (scope_id, tree_partition_id, id) on delete restrict,
  constraint frontstage_block_nodes_parent_tab_fkey foreign key (scope_id, tree_partition_id, tab_id, parent_id)
    references frontstage_block_nodes (scope_id, tree_partition_id, tab_id, id) on delete restrict,
  constraint frontstage_block_nodes_code_fkey foreign key (scope_id, tree_partition_id, code_ref)
    references frontstage_block_codes (workspace_id, page_id, code_ref)
    on delete restrict deferrable initially deferred
);

create index frontstage_block_nodes_siblings_idx
  on frontstage_block_nodes (scope_id, tree_partition_id, parent_id, sibling_rank, id);
create unique index frontstage_block_nodes_sibling_rank_uidx
  on frontstage_block_nodes (scope_id, tree_partition_id, parent_id, sibling_rank)
  where parent_id is not null;
create unique index frontstage_block_nodes_root_rank_uidx
  on frontstage_block_nodes (scope_id, tree_partition_id, sibling_rank)
  where parent_id is null;
create index frontstage_block_nodes_tab_idx
  on frontstage_block_nodes (scope_id, tree_partition_id, tab_id, sibling_rank, id);

with legacy_blocks as (
  select
    schemas.workspace_id,
    tabs.page_id,
    schemas.tab_id,
    schemas.created_at,
    schemas.updated_at,
    block.value as runtime_descriptor,
    block.ordinality,
    row_number() over (
      partition by schemas.workspace_id, tabs.page_id
      order by tabs.rank collate "C", tabs.id, block.ordinality
    ) as page_ordinality
  from frontstage_page_schemas schemas
  join frontstage_page_tabs tabs
    on tabs.workspace_id = schemas.workspace_id and tabs.id = schemas.tab_id
  cross join lateral jsonb_array_elements(
    case when jsonb_typeof(schemas.document_payload -> 'blocks') = 'array'
      then schemas.document_payload -> 'blocks' else '[]'::jsonb end
  )
    with ordinality as block(value, ordinality)
)
insert into frontstage_block_nodes (
  id, scope_id, tree_partition_id, parent_id, sibling_rank, block_id, tab_id,
  presentation, title, code_ref, schema_version, input_mapping, output_mapping,
  runtime_descriptor, created_at, updated_at
)
select
  gen_random_uuid(),
  workspace_id,
  page_id,
  null,
  lpad(page_ordinality::text, 20, '0') || 'U',
  runtime_descriptor ->> 'id',
  tab_id,
  'page',
  case when jsonb_typeof(runtime_descriptor -> 'title') = 'string'
    then runtime_descriptor ->> 'title' else null end,
  runtime_descriptor ->> 'codeRef',
  1,
  '{}'::jsonb,
  '{}'::jsonb,
  runtime_descriptor,
  created_at,
  updated_at
from legacy_blocks;
