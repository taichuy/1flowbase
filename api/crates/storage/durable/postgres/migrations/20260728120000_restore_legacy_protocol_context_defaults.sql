-- Old compilers persisted a JSON null when protocol_context was absent from a published draft.
-- Remove only those synthetic nulls so the runtime can apply its missing-field compatibility
-- default. Publication snapshots remain immutable and are the evidence for author intent.
with eligible_nodes as materialized (
    select distinct
        compiled_plans.id as compiled_plan_id,
        compiled_nodes.node_id
    from flow_compiled_plans compiled_plans
    cross join lateral jsonb_each(
        case
            when jsonb_typeof(compiled_plans.plan -> 'nodes') = 'object'
                then compiled_plans.plan -> 'nodes'
            else '{}'::jsonb
        end
    ) as compiled_nodes(node_id, node)
    join application_publication_versions publications
      on publications.compiled_plan_id = compiled_plans.id
    cross join lateral jsonb_array_elements(
        case
            when jsonb_typeof(publications.document_snapshot #> '{graph,nodes}') = 'array'
                then publications.document_snapshot #> '{graph,nodes}'
            else '[]'::jsonb
        end
    ) as snapshot_nodes(node)
    where compiled_nodes.node ->> 'node_id' = compiled_nodes.node_id
      and compiled_nodes.node ->> 'node_type' = 'llm'
      and (compiled_nodes.node -> 'config') ? 'protocol_context'
      and compiled_nodes.node #> '{config,protocol_context}' = 'null'::jsonb
      and snapshot_nodes.node ->> 'id' = compiled_nodes.node_id
      and snapshot_nodes.node ->> 'type' = 'llm'
      and not coalesce((snapshot_nodes.node -> 'config') ? 'protocol_context', false)
      and not exists (
          select 1
          from application_publication_versions conflicting_publications
          cross join lateral jsonb_array_elements(
              case
                  when jsonb_typeof(
                      conflicting_publications.document_snapshot #> '{graph,nodes}'
                  ) = 'array'
                      then conflicting_publications.document_snapshot #> '{graph,nodes}'
                  else '[]'::jsonb
              end
          ) as conflicting_nodes(node)
          where conflicting_publications.compiled_plan_id = compiled_plans.id
            and conflicting_nodes.node ->> 'id' = compiled_nodes.node_id
            and coalesce((conflicting_nodes.node -> 'config') ? 'protocol_context', false)
      )
)
update flow_compiled_plans compiled_plans
set plan = jsonb_set(
    compiled_plans.plan,
    '{nodes}',
    (
        select jsonb_object_agg(
            compiled_nodes.node_id,
            case
                when exists (
                    select 1
                    from eligible_nodes
                    where eligible_nodes.compiled_plan_id = compiled_plans.id
                      and eligible_nodes.node_id = compiled_nodes.node_id
                ) then
                    jsonb_set(
                        compiled_nodes.node,
                        '{config}',
                        (compiled_nodes.node -> 'config') - 'protocol_context',
                        false
                    )
                else compiled_nodes.node
            end
        )
        from jsonb_each(compiled_plans.plan -> 'nodes')
            as compiled_nodes(node_id, node)
    ),
    false
)
where exists (
    select 1
    from eligible_nodes
    where eligible_nodes.compiled_plan_id = compiled_plans.id
);
