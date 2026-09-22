-- Separate semantic proof identities; native predicates and indexes remain unchanged.
create index semantic_callback_response_identity
    on flow_run_callback_tasks ((request_payload #>> '{responses_round,response_id}'))
    where callback_kind = 'llm_tool_calls';
create index semantic_callback_call_identities
    on flow_run_callback_tasks using gin ((
        jsonb_path_query_array(request_payload, '$.tool_calls[*].call_id')
        || jsonb_path_query_array(request_payload, '$.tool_calls[*].id')
    ))
    where callback_kind = 'llm_tool_calls'
      and jsonb_typeof(request_payload #> '{responses_round}') = 'object';
