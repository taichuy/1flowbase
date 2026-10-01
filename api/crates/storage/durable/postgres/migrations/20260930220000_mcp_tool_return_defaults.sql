alter table mcp_tools
    add column max_inline_chars bigint check (max_inline_chars > 0),
    add column response_fields jsonb check (
        response_fields is null or jsonb_typeof(response_fields) = 'array'
    );
