update mcp_tools
set input_mapping = jsonb_set(
    input_mapping,
    '{mappings}',
    (
        select coalesce(
            jsonb_agg(
                case
                    when mapping.value ->> 'interface_param' = 'workspace_id' then
                        (mapping.value - 'mcp_param')
                        || '{"source":{"kind":"server_binding","binding":"workspace_id"}}'::jsonb
                    else mapping.value
                end
                order by mapping.ordinality
            ),
            '[]'::jsonb
        )
        from jsonb_array_elements(coalesce(input_mapping -> 'mappings', '[]'::jsonb))
             with ordinality as mapping(value, ordinality)
    )
)
where tool_id in (
    'frontstage_update_page_metadata',
    'frontstage_list_pages',
    'frontstage_create_tab',
    'frontstage_list_tabs',
    'frontstage_get_page_detail',
    'frontstage_create_page'
)
and exists (
    select 1
    from jsonb_array_elements(coalesce(input_mapping -> 'mappings', '[]'::jsonb)) as mapping(value)
    where mapping.value ->> 'interface_param' = 'workspace_id'
);
