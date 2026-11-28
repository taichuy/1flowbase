use super::*;
use control_plane_contracts::ports::McpToolSummaryRecord;

// Only static SQL fragments are interpolated; all query values remain bound.
const INSTANCE_FILTER: &str = r#"
    where workspace_id = $1
      and ($2::text is null or status = $2)
      and ($3::text is null or instance_id ilike $3 or name ilike $3
           or coalesce(description_short, '') ilike $3)
"#;

// Match the domain's boolean check, not a JSON string containing "true".
const TOOL_PAGE_SOURCE: &str = r#"
    with tool_summaries as (
        select id, workspace_id, tool_id, name, short_description, execution_kind, interface_id,
               upstream_connection_id, remote_tool_name, source_schema_hash,
               assistant_client_capability_code, permission_code, risk_level, des_id, status,
               revision, managed_bundle_organization, managed_bundle_id, managed_bundle_version,
               updated_at,
               exists (
                   select 1 from jsonb_array_elements(
                       case when jsonb_typeof(input_mapping->'mappings') = 'array'
                            then input_mapping->'mappings' else '[]'::jsonb end
                   ) as m
                   where m->>'interface_param' = 'des_id' and m->'source'->>'kind' = 'mcp_call'
                     and m->'required' = 'true'::jsonb
               ) as des_id_required
        from mcp_tools
        where workspace_id = $1
          and ($2::text is null or execution_kind = $2)
          and ($3::text is null or interface_id = $3)
          and ($4::text is null or risk_level = $4)
          and ($5::text is null or status = $5)
          and ($6::text[] is null or tool_id = any($6))
          -- The catalog searched "name tool_id operation source" as one literal substring.
          and ($7::text is null or concat_ws(' ', name, tool_id,
              case execution_kind
                  when 'interface_wrapper' then coalesce($8::jsonb ->> interface_id, interface_id)
                  when 'mcp_proxy' then 'MCP tools/call ' || remote_tool_name
                  when 'assistant_client' then 'Assistant browser capability ' || assistant_client_capability_code
              end,
              case execution_kind
                  when 'interface_wrapper' then interface_id
                  when 'mcp_proxy' then upstream_connection_id::text || ' / ' || remote_tool_name
                  when 'assistant_client' then assistant_client_capability_code
              end) ilike $7)
    )
"#;

impl PgControlPlaneStore {
    pub(super) async fn mcp_instance_summaries(
        &self,
        workspace_id: Uuid,
        status: Option<domain::McpInstanceStatus>,
        keyword: Option<&str>,
        page: Option<(usize, usize)>,
    ) -> Result<Vec<McpInstanceSummaryRecord>> {
        let offset = page.map(|(page, page_size)| page_offset(page, page_size));
        let size = page.map(|(_, page_size)| page_size as i64);
        let sql = format!(
            r#"
            with selected_instances as (
                select * from mcp_instances {INSTANCE_FILTER}
                order by updated_at desc, instance_id asc limit $4 offset $5
            )
            select i.*, coalesce(g.group_count, 0) as group_count, coalesce(b.tool_count, 0) as tool_count
            from selected_instances i
            left join (
                select instance_record_id, count(*) as group_count
                from mcp_groups join selected_instances on instance_record_id = selected_instances.id
                group by instance_record_id
            ) g on g.instance_record_id = i.id
            left join (
                select instance_record_id, count(*) as tool_count
                from mcp_tool_bindings join selected_instances on instance_record_id = selected_instances.id
                group by instance_record_id
            ) b on b.instance_record_id = i.id
            order by i.updated_at desc, i.instance_id asc
        "#
        );
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(workspace_id)
            .bind(status.map(|status| status.as_str()))
            .bind(keyword.map(keyword_pattern))
            .bind(size)
            .bind(offset)
            .fetch_all(self.pool())
            .await?;
        rows.into_iter()
            .map(|row| {
                Ok(McpInstanceSummaryRecord {
                    group_count: row.get("group_count"),
                    tool_count: row.get("tool_count"),
                    instance: map_instance(row)?,
                })
            })
            .collect()
    }

    pub(super) async fn mcp_instance_page(
        &self,
        workspace_id: Uuid,
        query: &McpInstancePageQuery,
    ) -> Result<McpInstancePage> {
        let keyword = query.keyword.as_deref();
        let total = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "select count(*) from mcp_instances {INSTANCE_FILTER}"
        )))
        .bind(workspace_id)
        .bind(query.status.map(|status| status.as_str()))
        .bind(keyword.map(keyword_pattern))
        .fetch_one(self.pool())
        .await?;
        let items = self
            .mcp_instance_summaries(
                workspace_id,
                query.status,
                keyword,
                Some((query.page, query.page_size)),
            )
            .await?;
        Ok(McpInstancePage {
            items,
            total,
            page: query.page,
            page_size: query.page_size,
        })
    }

    pub(super) async fn mcp_tool_page(
        &self,
        workspace_id: Uuid,
        query: &McpToolPageQuery,
    ) -> Result<McpToolPage> {
        let filter = "from tool_summaries where ($9::boolean is null or des_id_required = $9)";
        let count_sql = format!("{TOOL_PAGE_SOURCE} select count(*) {filter}");
        let page_sql = format!("{TOOL_PAGE_SOURCE} select * {filter} order by updated_at desc, tool_id asc limit $10 offset $11");
        let bind = |sql| {
            sqlx::query(sqlx::AssertSqlSafe(sql))
                .bind(workspace_id)
                .bind(query.execution_target_kind.as_deref())
                .bind(query.interface_id.as_deref())
                .bind(query.risk_level.as_deref())
                .bind(query.status.as_deref())
                .bind(query.tool_ids.as_deref())
                .bind(query.keyword.as_deref().map(keyword_pattern))
                .bind(sqlx::types::Json(&query.interface_operations))
                .bind(query.des_id_required)
        };
        let total = bind(count_sql).fetch_one(self.pool()).await?.get(0);
        let rows = bind(page_sql)
            .bind(query.page_size as i64)
            .bind(page_offset(query.page, query.page_size))
            .fetch_all(self.pool())
            .await?;
        let items = rows
            .into_iter()
            .map(|row| {
                Ok(McpToolSummaryRecord {
                    id: row.get("id"),
                    workspace_id: row.get("workspace_id"),
                    tool_id: row.get("tool_id"),
                    name: row.get("name"),
                    short_description: row.get("short_description"),
                    execution_target: map_execution_target(&row)?,
                    permission_code: row.get("permission_code"),
                    risk_level: parse_risk_level(row.get::<String, _>("risk_level").as_str())?,
                    des_id: row.get("des_id"),
                    des_id_required: row.get("des_id_required"),
                    status: parse_tool_status(row.get::<String, _>("status").as_str())?,
                    revision: row.get("revision"),
                    managed_by: managed_bundle_source(&row),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(McpToolPage {
            items,
            total,
            page: query.page,
            page_size: query.page_size,
        })
    }
}

fn keyword_pattern(keyword: &str) -> String {
    format!(
        "%{}%",
        keyword
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    )
}

fn page_offset(page: usize, page_size: usize) -> i64 {
    page.saturating_sub(1)
        .saturating_mul(page_size)
        .min(i64::MAX as usize) as i64
}
