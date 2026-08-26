-- Rename the persisted LLM output limit to the canonical runtime contract.
-- Published document snapshots are immutable and intentionally excluded.
do $$
begin
    if exists (
        select 1
        from flow_drafts drafts
        cross join lateral jsonb_array_elements(
            case
                when jsonb_typeof(drafts.document #> '{graph,nodes}') = 'array'
                    then drafts.document #> '{graph,nodes}'
                else '[]'::jsonb
            end
        ) as draft_nodes(node)
        where (draft_nodes.node #> '{config,llm_parameters,items}') ? 'max_tokens'
          and (draft_nodes.node #> '{config,llm_parameters,items}') ? 'max_output_tokens'

        union all

        select 1
        from flow_versions versions
        cross join lateral jsonb_array_elements(
            case
                when jsonb_typeof(versions.document #> '{graph,nodes}') = 'array'
                    then versions.document #> '{graph,nodes}'
                else '[]'::jsonb
            end
        ) as version_nodes(node)
        where (version_nodes.node #> '{config,llm_parameters,items}') ? 'max_tokens'
          and (version_nodes.node #> '{config,llm_parameters,items}') ? 'max_output_tokens'

        union all

        select 1
        from flow_compiled_plans compiled_plans
        cross join lateral jsonb_each(
            case
                when jsonb_typeof(compiled_plans.plan -> 'nodes') = 'object'
                    then compiled_plans.plan -> 'nodes'
                else '{}'::jsonb
            end
        ) as compiled_nodes(node_id, node)
        where (compiled_nodes.node #> '{config,llm_parameters,items}') ? 'max_tokens'
          and (compiled_nodes.node #> '{config,llm_parameters,items}') ? 'max_output_tokens'
    ) then
        raise exception using
            errcode = 'check_violation',
            message = 'LLM max output token migration rejected an item containing both max_tokens and max_output_tokens';
    end if;
end
$$;

update flow_drafts drafts
set document = jsonb_set(
    drafts.document,
    '{graph,nodes}',
    (
        select jsonb_agg(
            case
                when (draft_nodes.node #> '{config,llm_parameters,items}') ? 'max_tokens' then
                    jsonb_set(
                        draft_nodes.node,
                        '{config,llm_parameters,items}',
                        ((draft_nodes.node #> '{config,llm_parameters,items}') - 'max_tokens')
                            || jsonb_build_object(
                                'max_output_tokens',
                                draft_nodes.node #> '{config,llm_parameters,items,max_tokens}'
                            ),
                        false
                    )
                else draft_nodes.node
            end
            order by draft_nodes.ordinality
        )
        from jsonb_array_elements(drafts.document #> '{graph,nodes}')
            with ordinality as draft_nodes(node, ordinality)
    ),
    false
)
where jsonb_typeof(drafts.document #> '{graph,nodes}') = 'array'
  and exists (
      select 1
      from jsonb_array_elements(drafts.document #> '{graph,nodes}') as draft_nodes(node)
      where (draft_nodes.node #> '{config,llm_parameters,items}') ? 'max_tokens'
  );

update flow_versions versions
set document = jsonb_set(
    versions.document,
    '{graph,nodes}',
    (
        select jsonb_agg(
            case
                when (version_nodes.node #> '{config,llm_parameters,items}') ? 'max_tokens' then
                    jsonb_set(
                        version_nodes.node,
                        '{config,llm_parameters,items}',
                        ((version_nodes.node #> '{config,llm_parameters,items}') - 'max_tokens')
                            || jsonb_build_object(
                                'max_output_tokens',
                                version_nodes.node #> '{config,llm_parameters,items,max_tokens}'
                            ),
                        false
                    )
                else version_nodes.node
            end
            order by version_nodes.ordinality
        )
        from jsonb_array_elements(versions.document #> '{graph,nodes}')
            with ordinality as version_nodes(node, ordinality)
    ),
    false
)
where jsonb_typeof(versions.document #> '{graph,nodes}') = 'array'
  and exists (
      select 1
      from jsonb_array_elements(versions.document #> '{graph,nodes}') as version_nodes(node)
      where (version_nodes.node #> '{config,llm_parameters,items}') ? 'max_tokens'
  );

update flow_compiled_plans compiled_plans
set plan = jsonb_set(
    compiled_plans.plan,
    '{nodes}',
    (
        select jsonb_object_agg(
            compiled_nodes.node_id,
            case
                when (compiled_nodes.node #> '{config,llm_parameters,items}') ? 'max_tokens' then
                    jsonb_set(
                        compiled_nodes.node,
                        '{config,llm_parameters,items}',
                        ((compiled_nodes.node #> '{config,llm_parameters,items}') - 'max_tokens')
                            || jsonb_build_object(
                                'max_output_tokens',
                                compiled_nodes.node #> '{config,llm_parameters,items,max_tokens}'
                            ),
                        false
                    )
                else compiled_nodes.node
            end
        )
        from jsonb_each(compiled_plans.plan -> 'nodes') as compiled_nodes(node_id, node)
    ),
    false
)
where jsonb_typeof(compiled_plans.plan -> 'nodes') = 'object'
  and exists (
      select 1
      from jsonb_each(compiled_plans.plan -> 'nodes') as compiled_nodes(node_id, node)
      where (compiled_nodes.node #> '{config,llm_parameters,items}') ? 'max_tokens'
  );
