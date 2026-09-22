-- Correlation reads must not detoast every historical callback payload on each
-- Responses turn. Index only response/call identities, never tool arguments.
create index native_callback_response_identity
    on flow_run_callback_tasks ((request_payload #>> '{provider_metadata,native_response,response_id}'))
    where callback_kind = 'llm_tool_calls';

create index native_callback_call_identities
    on flow_run_callback_tasks using gin ((
        jsonb_path_query_array(request_payload, '$.tool_calls[*].call_id')
        || jsonb_path_query_array(request_payload, '$.tool_calls[*].id')
    ))
    where callback_kind = 'llm_tool_calls'
      and jsonb_typeof(request_payload #> '{provider_metadata,native_response}') = 'object';
