alter table application_api_mappings
    add column operation_bindings jsonb not null default '{"generate":null,"count_tokens":null,"compact":{"responses_compact":null,"responses_compaction_v2":null}}'::jsonb;

alter table application_publication_versions
    add column operation_bindings jsonb not null default '{"generate":null,"count_tokens":null,"compact":{"responses_compact":null,"responses_compaction_v2":null}}'::jsonb;

with runnable_llm_targets as (
    select
        publication.id as publication_id,
        node.value ->> 'node_id' as target_node_id
    from application_publication_versions publication
    join flow_compiled_plans compiled_plan
      on compiled_plan.id = publication.compiled_plan_id
    cross join lateral jsonb_each(
        case
            when jsonb_typeof(compiled_plan.plan -> 'nodes') = 'object'
                then compiled_plan.plan -> 'nodes'
            else '{}'::jsonb
        end
    ) as node(key, value)
    where node.key = node.value ->> 'node_id'
      and nullif(btrim(node.value ->> 'node_id'), '') is not null
      and node.value ->> 'node_id' = btrim(node.value ->> 'node_id')
      and node.value ->> 'node_type' = 'llm'
      and jsonb_typeof(node.value -> 'llm_runtime') = 'object'
      and nullif(
          btrim(node.value #>> '{llm_runtime,provider_instance_id}'),
          ''
      ) is not null
      and node.value #>> '{llm_runtime,provider_instance_id}' =
          btrim(node.value #>> '{llm_runtime,provider_instance_id}')
      and nullif(
          btrim(node.value #>> '{llm_runtime,provider_code}'),
          ''
      ) is not null
      and node.value #>> '{llm_runtime,provider_code}' =
          btrim(node.value #>> '{llm_runtime,provider_code}')
      and nullif(btrim(node.value #>> '{llm_runtime,protocol}'), '') is not null
      and node.value #>> '{llm_runtime,protocol}' =
          btrim(node.value #>> '{llm_runtime,protocol}')
      and nullif(btrim(node.value #>> '{llm_runtime,model}'), '') is not null
      and node.value #>> '{llm_runtime,model}' =
          btrim(node.value #>> '{llm_runtime,model}')
), unique_generate_preview as (
    select
        publication_id,
        min(target_node_id) as target_node_id
    from runnable_llm_targets
    group by publication_id
    having count(*) = 1
)
update application_publication_versions publication
set operation_bindings = jsonb_build_object(
    'generate', jsonb_build_object('target_node_id', preview.target_node_id),
    'count_tokens', null,
    'compact', jsonb_build_object(
        'responses_compact', null,
        'responses_compaction_v2', null
    )
)
from unique_generate_preview preview
where publication.id = preview.publication_id;
