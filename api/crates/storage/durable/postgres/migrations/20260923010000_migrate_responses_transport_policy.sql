-- Editable flow documents use the three-state node policy. Published snapshots
-- and compiled plans retain their original bytes; runtime reads their legacy key.
update flow_drafts drafts
set document = jsonb_set(
    drafts.document,
    '{graph,nodes}',
    (
        select jsonb_agg(
            case
                when (draft_nodes.node #> '{config,llm_parameters,items}') ? 'use_responses_websocket' then
                    jsonb_set(
                        draft_nodes.node,
                        '{config,llm_parameters,items}',
                        ((draft_nodes.node #> '{config,llm_parameters,items}') - 'use_responses_websocket')
                            || case
                                when (draft_nodes.node #> '{config,llm_parameters,items}') ? 'responses_transport_policy'
                                    then '{}'::jsonb
                                else jsonb_build_object(
                                    'responses_transport_policy',
                                    (draft_nodes.node #> '{config,llm_parameters,items,use_responses_websocket}')
                                        || jsonb_build_object(
                                            'enabled', true,
                                            'value', case
                                                when draft_nodes.node #> '{config,llm_parameters,items,use_responses_websocket,enabled}' = 'true'::jsonb
                                                 and draft_nodes.node #> '{config,llm_parameters,items,use_responses_websocket,value}' = 'true'::jsonb
                                                    then 'force_websocket'
                                                else 'inherit'
                                            end
                                        )
                                )
                            end,
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
      where (draft_nodes.node #> '{config,llm_parameters,items}') ? 'use_responses_websocket'
  );

update flow_versions versions
set document = jsonb_set(
    versions.document,
    '{graph,nodes}',
    (
        select jsonb_agg(
            case
                when (version_nodes.node #> '{config,llm_parameters,items}') ? 'use_responses_websocket' then
                    jsonb_set(
                        version_nodes.node,
                        '{config,llm_parameters,items}',
                        ((version_nodes.node #> '{config,llm_parameters,items}') - 'use_responses_websocket')
                            || case
                                when (version_nodes.node #> '{config,llm_parameters,items}') ? 'responses_transport_policy'
                                    then '{}'::jsonb
                                else jsonb_build_object(
                                    'responses_transport_policy',
                                    (version_nodes.node #> '{config,llm_parameters,items,use_responses_websocket}')
                                        || jsonb_build_object(
                                            'enabled', true,
                                            'value', case
                                                when version_nodes.node #> '{config,llm_parameters,items,use_responses_websocket,enabled}' = 'true'::jsonb
                                                 and version_nodes.node #> '{config,llm_parameters,items,use_responses_websocket,value}' = 'true'::jsonb
                                                    then 'force_websocket'
                                                else 'inherit'
                                            end
                                        )
                                )
                            end,
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
      where (version_nodes.node #> '{config,llm_parameters,items}') ? 'use_responses_websocket'
  );
