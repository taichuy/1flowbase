-- Move the mutable authoring/runtime selector to the typed Start variable. Historical flow
-- versions and publication document snapshots are intentionally immutable.
update flow_drafts drafts
set document = jsonb_set(
    drafts.document,
    '{graph,nodes}',
    (
        select jsonb_agg(
            case
                when node ->> 'type' = 'llm'
                  and node #> '{config,protocol_context,kind}' = '"selector"'::jsonb
                  and node #> '{config,protocol_context,value}' = '["sys","protocol_context"]'::jsonb
                then jsonb_set(
                    node,
                    '{config,protocol_context,value}',
                    jsonb_build_array(start_node.node_id, 'protocol_context'),
                    false
                )
                else node
            end
            order by node_ordinality
        )
        from jsonb_array_elements(drafts.document #> '{graph,nodes}')
            with ordinality as graph_nodes(node, node_ordinality)
        cross join lateral (
            select candidate ->> 'id' as node_id
            from jsonb_array_elements(drafts.document #> '{graph,nodes}') candidate
            where candidate ->> 'type' = 'start'
            limit 1
        ) start_node
    ),
    false
)
where jsonb_typeof(drafts.document #> '{graph,nodes}') = 'array'
  and exists (
      select 1
      from jsonb_array_elements(drafts.document #> '{graph,nodes}') node
      where node ->> 'type' = 'start'
  )
  and exists (
      select 1
      from jsonb_array_elements(drafts.document #> '{graph,nodes}') node
      where node ->> 'type' = 'llm'
        and node #> '{config,protocol_context,value}' = '["sys","protocol_context"]'::jsonb
  );

update flow_compiled_plans compiled_plans
set plan = jsonb_set(
    compiled_plans.plan,
    '{nodes}',
    (
        select jsonb_object_agg(
            compiled_nodes.node_id,
            case
                when node ->> 'node_type' = 'llm'
                  and node #> '{config,protocol_context,kind}' = '"selector"'::jsonb
                  and node #> '{config,protocol_context,value}' = '["sys","protocol_context"]'::jsonb
                then jsonb_set(
                    node,
                    '{config,protocol_context,value}',
                    jsonb_build_array(start_node.node_id, 'protocol_context'),
                    false
                )
                else node
            end
        )
        from jsonb_each(compiled_plans.plan -> 'nodes') compiled_nodes(node_id, node)
        cross join lateral (
            select candidate_id as node_id
            from jsonb_each(compiled_plans.plan -> 'nodes') candidates(candidate_id, candidate)
            where candidate ->> 'node_type' = 'start'
            limit 1
        ) start_node
    ),
    false
)
where jsonb_typeof(compiled_plans.plan -> 'nodes') = 'object'
  and exists (
      select 1
      from jsonb_each(compiled_plans.plan -> 'nodes') compiled_nodes(node_id, node)
      where node ->> 'node_type' = 'start'
  )
  and exists (
      select 1
      from jsonb_each(compiled_plans.plan -> 'nodes') compiled_nodes(node_id, node)
      where node ->> 'node_type' = 'llm'
        and node #> '{config,protocol_context,value}' = '["sys","protocol_context"]'::jsonb
  );
