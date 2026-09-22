with recursive roots as (
    select t.*, t.created_by as user_id
    from application_run_log_tasks t
    where t.application_id = $1 and t.is_root
      and ($2::timestamptz is null or t.started_at >= $2)
      and ($3::timestamptz is null or t.started_at < $3)
), tree(root_id, task_id) as (
    select id, id from roots
    union
    select tree.root_id, child.id from tree
    join application_run_log_tasks child on child.parent_task_run_id = tree.task_id
    where child.application_id = $1
), usage as (
    select tree.root_id,
        sum(t.total_tokens)::bigint as total_tokens,
        sum(t.input_tokens)::bigint as input_tokens,
        sum(t.output_tokens)::bigint as output_tokens,
        sum(t.input_cache_hit_tokens)::bigint as input_cache_hit_tokens,
        sum(t.total_cost::numeric)::double precision as total_cost,
        bool_or(t.total_cost is null) as cost_incomplete,
        sum(t.unique_node_count)::bigint as unique_node_count,
        sum(t.tool_callback_count)::bigint as tool_callback_count
    from tree join application_run_log_tasks t on t.id = tree.task_id
    group by tree.root_id
), monitoring_logs as (
    select r.id as flow_run_id, r.title, r.status, r.run_mode,
        r.started_at, r.finished_at, r.compatibility_mode,
        r.authorized_account, r.api_key_id, r.api_key_name_snapshot,
        r.external_conversation_id, r.requested_model_id, r.user_id, u.name,
        usage.total_tokens, usage.input_tokens, usage.output_tokens,
        usage.input_cache_hit_tokens, usage.total_cost, usage.cost_incomplete,
        usage.unique_node_count, usage.tool_callback_count
    from roots r join usage on usage.root_id = r.id
    left join users u on u.id = r.user_id
)
