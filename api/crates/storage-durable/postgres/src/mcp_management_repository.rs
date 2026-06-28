use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        AuthRepository, CreateMcpInstanceInput, CreateMcpToolBindingInput, CreateMcpToolInput,
        McpManagementRepository, UpdateMcpInstanceInput, UpdateMcpMetaToolConfigInput,
        UpdateMcpToolBindingInput, UpdateMcpToolInput, UpsertMcpGroupInput,
    },
};
use sqlx::Row;
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

fn parse_instance_status(value: &str) -> Result<domain::McpInstanceStatus> {
    match value {
        "draft" => Ok(domain::McpInstanceStatus::Draft),
        "enabled" => Ok(domain::McpInstanceStatus::Enabled),
        "disabled" => Ok(domain::McpInstanceStatus::Disabled),
        "archived" => Ok(domain::McpInstanceStatus::Archived),
        _ => anyhow::bail!("invalid MCP instance status"),
    }
}

fn parse_tool_status(value: &str) -> Result<domain::McpToolStatus> {
    match value {
        "draft" => Ok(domain::McpToolStatus::Draft),
        "enabled" => Ok(domain::McpToolStatus::Enabled),
        "disabled" => Ok(domain::McpToolStatus::Disabled),
        "archived" => Ok(domain::McpToolStatus::Archived),
        _ => anyhow::bail!("invalid MCP tool status"),
    }
}

fn parse_risk_level(value: &str) -> Result<domain::McpRiskLevel> {
    match value {
        "low" => Ok(domain::McpRiskLevel::Low),
        "medium" => Ok(domain::McpRiskLevel::Medium),
        "high" => Ok(domain::McpRiskLevel::High),
        "critical" => Ok(domain::McpRiskLevel::Critical),
        _ => anyhow::bail!("invalid MCP risk level"),
    }
}

fn map_instance(row: sqlx::postgres::PgRow) -> Result<domain::McpInstanceRecord> {
    Ok(domain::McpInstanceRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        instance_id: row.get("instance_id"),
        name: row.get("name"),
        description_short: row.get("description_short"),
        status: parse_instance_status(row.get::<String, _>("status").as_str())?,
        default_entry_path: row.get("default_entry_path"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_group(row: sqlx::postgres::PgRow) -> Result<domain::McpGroupRecord> {
    Ok(domain::McpGroupRecord {
        id: row.get("id"),
        instance_record_id: row.get("instance_record_id"),
        path: row.get("path"),
        display_name: row.get("display_name"),
        description_short: row.get("description_short"),
        enabled: row.get("enabled"),
        sort_order: row.get("sort_order"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_tool(row: sqlx::postgres::PgRow) -> Result<domain::McpToolRecord> {
    Ok(domain::McpToolRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        tool_id: row.get("tool_id"),
        name: row.get("name"),
        short_description: row.get("short_description"),
        usage_description: row.get("usage_description"),
        full_description: row.get("full_description"),
        interface_id: row.get("interface_id"),
        parameter_schema: row.get("parameter_schema"),
        result_schema: row.get("result_schema"),
        input_mapping: row.get("input_mapping"),
        output_mapping: row.get("output_mapping"),
        permission_code: row.get("permission_code"),
        risk_level: parse_risk_level(row.get::<String, _>("risk_level").as_str())?,
        audit_policy: row.get("audit_policy"),
        des_id: row.get("des_id"),
        des_id_required: row.get("des_id_required"),
        status: parse_tool_status(row.get::<String, _>("status").as_str())?,
        revision: row.get("revision"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_binding(row: sqlx::postgres::PgRow) -> Result<domain::McpToolBindingRecord> {
    Ok(domain::McpToolBindingRecord {
        id: row.get("id"),
        instance_record_id: row.get("instance_record_id"),
        tool_record_id: row.get("tool_record_id"),
        group_path: row.get("group_path"),
        tool_id: row.get("tool_id"),
        display_alias: row.get("display_alias"),
        visible: row.get("visible"),
        sort_order: row.get("sort_order"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_meta_tool_config(row: sqlx::postgres::PgRow) -> Result<domain::McpMetaToolConfigRecord> {
    Ok(domain::McpMetaToolConfigRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        list_default_limit: row.get("list_default_limit"),
        list_max_depth: row.get("list_max_depth"),
        list_regex_enabled: row.get("list_regex_enabled"),
        list_regex_max_length: row.get("list_regex_max_length"),
        list_return_fields: row.get("list_return_fields"),
        get_include_mapping_summary: row.get("get_include_mapping_summary"),
        get_include_interface_summary: row.get("get_include_interface_summary"),
        call_default_des_id_policy: row.get("call_default_des_id_policy"),
        call_high_risk_requires_des_id: row.get("call_high_risk_requires_des_id"),
        call_validation_error_format: row.get("call_validation_error_format"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

#[async_trait]
impl McpManagementRepository for PgControlPlaneStore {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        AuthRepository::load_actor_context_for_user(self, actor_user_id).await
    }

    async fn list_mcp_instances(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::McpInstanceRecord>> {
        let rows = sqlx::query(
            r#"
            select *
            from mcp_instances
            where workspace_id = $1
            order by updated_at desc, instance_id asc
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_instance).collect()
    }

    async fn get_mcp_instance(
        &self,
        workspace_id: Uuid,
        instance_id: &str,
    ) -> Result<Option<domain::McpInstanceRecord>> {
        let row = sqlx::query(
            r#"
            select *
            from mcp_instances
            where workspace_id = $1 and instance_id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(instance_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_instance).transpose()
    }

    async fn create_mcp_instance(
        &self,
        input: &CreateMcpInstanceInput,
    ) -> Result<domain::McpInstanceRecord> {
        let row = sqlx::query(
            r#"
            insert into mcp_instances (
                id,
                workspace_id,
                instance_id,
                name,
                description_short,
                status,
                default_entry_path,
                created_by,
                updated_by
            ) values (
                $1, $2, $3, $4, $5, $6, $7, $8, $8
            )
            returning *
            "#,
        )
        .bind(input.id)
        .bind(input.workspace_id)
        .bind(&input.instance_id)
        .bind(&input.name)
        .bind(&input.description_short)
        .bind(input.status.as_str())
        .bind(&input.default_entry_path)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_instance(row)
    }

    async fn update_mcp_instance(
        &self,
        input: &UpdateMcpInstanceInput,
    ) -> Result<domain::McpInstanceRecord> {
        let row = sqlx::query(
            r#"
            update mcp_instances
            set
                name = $3,
                description_short = $4,
                status = $5,
                default_entry_path = $6,
                updated_by = $7,
                updated_at = now()
            where workspace_id = $1 and instance_id = $2
            returning *
            "#,
        )
        .bind(input.workspace_id)
        .bind(&input.instance_id)
        .bind(&input.name)
        .bind(&input.description_short)
        .bind(input.status.as_str())
        .bind(&input.default_entry_path)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_instance(row)
    }

    async fn delete_mcp_instance(&self, workspace_id: Uuid, instance_id: &str) -> Result<()> {
        sqlx::query("delete from mcp_instances where workspace_id = $1 and instance_id = $2")
            .bind(workspace_id)
            .bind(instance_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    async fn list_mcp_groups(
        &self,
        instance_record_ids: &[Uuid],
    ) -> Result<Vec<domain::McpGroupRecord>> {
        if instance_record_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            r#"
            select *
            from mcp_groups
            where instance_record_id = any($1)
            order by sort_order asc, path asc
            "#,
        )
        .bind(instance_record_ids)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_group).collect()
    }

    async fn upsert_mcp_group(
        &self,
        input: &UpsertMcpGroupInput,
    ) -> Result<domain::McpGroupRecord> {
        let row = sqlx::query(
            r#"
            insert into mcp_groups (
                id,
                instance_record_id,
                path,
                display_name,
                description_short,
                enabled,
                sort_order,
                scope_id,
                created_by,
                updated_by
            ) values (
                $1, $2, $3, $4, $5, $6, $7, (select scope_id from mcp_instances where id = $2), $8, $8
            )
            on conflict (instance_record_id, path) do update
            set
                display_name = excluded.display_name,
                description_short = excluded.description_short,
                enabled = excluded.enabled,
                sort_order = excluded.sort_order,
                updated_by = excluded.updated_by,
                updated_at = now()
            returning *
            "#,
        )
        .bind(input.id)
        .bind(input.instance_record_id)
        .bind(&input.path)
        .bind(&input.display_name)
        .bind(&input.description_short)
        .bind(input.enabled)
        .bind(input.sort_order)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_group(row)
    }

    async fn update_mcp_group(
        &self,
        input: &UpsertMcpGroupInput,
    ) -> Result<domain::McpGroupRecord> {
        let row = sqlx::query(
            r#"
            update mcp_groups
            set
                display_name = $3,
                description_short = $4,
                enabled = $5,
                sort_order = $6,
                updated_by = $7,
                updated_at = now()
            where instance_record_id = $1 and path = $2
            returning *
            "#,
        )
        .bind(input.instance_record_id)
        .bind(&input.path)
        .bind(&input.display_name)
        .bind(&input.description_short)
        .bind(input.enabled)
        .bind(input.sort_order)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_group(row)
    }

    async fn delete_mcp_group(&self, group_id: Uuid) -> Result<()> {
        sqlx::query("delete from mcp_groups where id = $1")
            .bind(group_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    async fn list_mcp_tools(&self, workspace_id: Uuid) -> Result<Vec<domain::McpToolRecord>> {
        let rows = sqlx::query(
            r#"
            select *
            from mcp_tools
            where workspace_id = $1
            order by updated_at desc, tool_id asc
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_tool).collect()
    }

    async fn get_mcp_tool(
        &self,
        workspace_id: Uuid,
        tool_id: &str,
    ) -> Result<Option<domain::McpToolRecord>> {
        let row = sqlx::query(
            r#"
            select *
            from mcp_tools
            where workspace_id = $1 and tool_id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(tool_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_tool).transpose()
    }

    async fn create_mcp_tool(&self, input: &CreateMcpToolInput) -> Result<domain::McpToolRecord> {
        let row = sqlx::query(
            r#"
            insert into mcp_tools (
                id,
                workspace_id,
                tool_id,
                name,
                short_description,
                usage_description,
                full_description,
                interface_id,
                parameter_schema,
                result_schema,
                input_mapping,
                output_mapping,
                permission_code,
                risk_level,
                audit_policy,
                des_id,
                des_id_required,
                status,
                created_by,
                updated_by
            ) values (
                $1, $2, $3, $4, $5, $6, $7, $8, $9,
                $10, $11, $12, $13, $14, $15, $16, $17,
                $18, $19, $19
            )
            returning *
            "#,
        )
        .bind(input.id)
        .bind(input.workspace_id)
        .bind(&input.tool_id)
        .bind(&input.name)
        .bind(&input.short_description)
        .bind(&input.usage_description)
        .bind(&input.full_description)
        .bind(&input.interface_id)
        .bind(&input.parameter_schema)
        .bind(&input.result_schema)
        .bind(&input.input_mapping)
        .bind(&input.output_mapping)
        .bind(&input.permission_code)
        .bind(input.risk_level.as_str())
        .bind(&input.audit_policy)
        .bind(&input.des_id)
        .bind(input.des_id_required)
        .bind(input.status.as_str())
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_tool(row)
    }

    async fn update_mcp_tool(&self, input: &UpdateMcpToolInput) -> Result<domain::McpToolRecord> {
        let row = sqlx::query(
            r#"
            update mcp_tools
            set
                name = $3,
                short_description = $4,
                usage_description = $5,
                full_description = $6,
                interface_id = $7,
                parameter_schema = $8,
                result_schema = $9,
                input_mapping = $10,
                output_mapping = $11,
                permission_code = $12,
                risk_level = $13,
                audit_policy = $14,
                des_id = $15,
                des_id_required = $16,
                status = $17,
                revision = revision + 1,
                updated_by = $18,
                updated_at = now()
            where workspace_id = $1 and tool_id = $2
            returning *
            "#,
        )
        .bind(input.workspace_id)
        .bind(&input.tool_id)
        .bind(&input.name)
        .bind(&input.short_description)
        .bind(&input.usage_description)
        .bind(&input.full_description)
        .bind(&input.interface_id)
        .bind(&input.parameter_schema)
        .bind(&input.result_schema)
        .bind(&input.input_mapping)
        .bind(&input.output_mapping)
        .bind(&input.permission_code)
        .bind(input.risk_level.as_str())
        .bind(&input.audit_policy)
        .bind(&input.des_id)
        .bind(input.des_id_required)
        .bind(input.status.as_str())
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_tool(row)
    }

    async fn refresh_mcp_tool_des_id(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        tool_id: &str,
        des_id: &str,
    ) -> Result<domain::McpToolRecord> {
        let row = sqlx::query(
            r#"
            update mcp_tools
            set
                des_id = $3,
                revision = revision + 1,
                updated_by = $4,
                updated_at = now()
            where workspace_id = $1 and tool_id = $2
            returning *
            "#,
        )
        .bind(workspace_id)
        .bind(tool_id)
        .bind(des_id)
        .bind(actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_tool(row)
    }

    async fn delete_mcp_tool(&self, workspace_id: Uuid, tool_id: &str) -> Result<()> {
        sqlx::query("delete from mcp_tools where workspace_id = $1 and tool_id = $2")
            .bind(workspace_id)
            .bind(tool_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    async fn list_mcp_tool_bindings(
        &self,
        instance_record_ids: &[Uuid],
    ) -> Result<Vec<domain::McpToolBindingRecord>> {
        if instance_record_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            r#"
            select
                mcp_tool_bindings.*,
                mcp_tools.tool_id
            from mcp_tool_bindings
            join mcp_tools on mcp_tools.id = mcp_tool_bindings.tool_record_id
            where mcp_tool_bindings.instance_record_id = any($1)
            order by mcp_tool_bindings.group_path asc, mcp_tool_bindings.sort_order asc, mcp_tool_bindings.id asc
            "#,
        )
        .bind(instance_record_ids)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_binding).collect()
    }

    async fn create_mcp_tool_binding(
        &self,
        input: &CreateMcpToolBindingInput,
    ) -> Result<domain::McpToolBindingRecord> {
        let row = sqlx::query(
            r#"
            insert into mcp_tool_bindings (
                id,
                instance_record_id,
                tool_record_id,
                group_path,
                display_alias,
                visible,
                sort_order,
                scope_id,
                created_by,
                updated_by
            ) values (
                $1, $2, $3, $4, $5, $6, $7, (select scope_id from mcp_instances where id = $2), $8, $8
            )
            returning
                mcp_tool_bindings.*,
                (
                    select tool_id
                    from mcp_tools
                    where mcp_tools.id = mcp_tool_bindings.tool_record_id
                ) as tool_id
            "#,
        )
        .bind(input.id)
        .bind(input.instance_record_id)
        .bind(input.tool_record_id)
        .bind(&input.group_path)
        .bind(&input.display_alias)
        .bind(input.visible)
        .bind(input.sort_order)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_binding(row)
    }

    async fn update_mcp_tool_binding(
        &self,
        input: &UpdateMcpToolBindingInput,
    ) -> Result<domain::McpToolBindingRecord> {
        let row = sqlx::query(
            r#"
            update mcp_tool_bindings
            set
                group_path = $2,
                display_alias = $3,
                visible = $4,
                sort_order = $5,
                updated_by = $6,
                updated_at = now()
            where id = $1
              and exists (
                  select 1
                  from mcp_instances
                  where mcp_instances.id = mcp_tool_bindings.instance_record_id
                    and mcp_instances.workspace_id = $7
              )
            returning
                mcp_tool_bindings.*,
                (
                    select tool_id
                    from mcp_tools
                    where mcp_tools.id = mcp_tool_bindings.tool_record_id
                ) as tool_id
            "#,
        )
        .bind(input.binding_id)
        .bind(&input.group_path)
        .bind(&input.display_alias)
        .bind(input.visible)
        .bind(input.sort_order)
        .bind(input.actor_user_id)
        .bind(input.workspace_id)
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::NotFound("mcp_tool_binding"))?;

        map_binding(row)
    }

    async fn delete_mcp_tool_binding(&self, workspace_id: Uuid, binding_id: Uuid) -> Result<()> {
        let result = sqlx::query(
            r#"
            delete from mcp_tool_bindings
            using mcp_instances
            where mcp_tool_bindings.id = $1
              and mcp_tool_bindings.instance_record_id = mcp_instances.id
              and mcp_instances.workspace_id = $2
            "#,
        )
        .bind(binding_id)
        .bind(workspace_id)
        .execute(self.pool())
        .await?;
        if result.rows_affected() == 0 {
            return Err(ControlPlaneError::NotFound("mcp_tool_binding").into());
        }
        Ok(())
    }

    async fn get_mcp_meta_tool_config(
        &self,
        workspace_id: Uuid,
    ) -> Result<Option<domain::McpMetaToolConfigRecord>> {
        let row = sqlx::query(
            r#"
            select *
            from mcp_meta_tool_configs
            where workspace_id = $1
            "#,
        )
        .bind(workspace_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_meta_tool_config).transpose()
    }

    async fn create_default_mcp_meta_tool_config(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<domain::McpMetaToolConfigRecord> {
        let row = sqlx::query(
            r#"
            insert into mcp_meta_tool_configs (
                id,
                workspace_id,
                created_by,
                updated_by
            ) values (
                $1, $2, $3, $3
            )
            on conflict (workspace_id) do update
            set updated_at = mcp_meta_tool_configs.updated_at
            returning *
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(workspace_id)
        .bind(actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_meta_tool_config(row)
    }

    async fn update_mcp_meta_tool_config(
        &self,
        input: &UpdateMcpMetaToolConfigInput,
    ) -> Result<domain::McpMetaToolConfigRecord> {
        let row = sqlx::query(
            r#"
            update mcp_meta_tool_configs
            set
                list_default_limit = $2,
                list_max_depth = $3,
                list_regex_enabled = $4,
                list_regex_max_length = $5,
                list_return_fields = $6,
                get_include_mapping_summary = $7,
                get_include_interface_summary = $8,
                call_default_des_id_policy = $9,
                call_high_risk_requires_des_id = $10,
                call_validation_error_format = $11,
                updated_by = $12,
                updated_at = now()
            where workspace_id = $1
            returning *
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.list_default_limit)
        .bind(input.list_max_depth)
        .bind(input.list_regex_enabled)
        .bind(input.list_regex_max_length)
        .bind(&input.list_return_fields)
        .bind(input.get_include_mapping_summary)
        .bind(input.get_include_interface_summary)
        .bind(&input.call_default_des_id_policy)
        .bind(input.call_high_risk_requires_des_id)
        .bind(&input.call_validation_error_format)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_meta_tool_config(row)
    }
}
