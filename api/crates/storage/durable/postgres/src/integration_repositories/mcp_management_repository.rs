use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::{
    ports::{
        AuthRepository, CreateMcpInstanceGraphInput, CreateMcpInstanceInput,
        CreateMcpToolBindingInput, CreateMcpToolInput, CreateMcpUpstreamConnectionInput,
        McpManagementRepository, ReplaceMcpBundleGraphInput, UpdateMcpInstanceDiscoveryPolicyInput,
        UpdateMcpInstanceInput, UpdateMcpToolBindingInput, UpdateMcpToolInput,
        UpdateMcpUpstreamConnectionInput, UpsertMcpClientCredentialInput, UpsertMcpGroupInput,
        UpsertMcpUpstreamSecretInput, UpsertMcpUpstreamToolSourceInput,
    },
    ControlPlaneContractError as ControlPlaneError,
};
use serde_json::json;
use sqlx::Row;

use crate::secret_crypto::{decrypt_secret_json, encrypt_secret_json};
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

#[path = "mcp_management_repository/bundle_graph.rs"]
mod bundle_graph;
#[path = "mcp_management_repository/managed_bundle_graph.rs"]
mod managed_bundle_graph;
#[path = "mcp_management_repository/mappers.rs"]
mod mappers;

use mappers::*;

#[async_trait]
impl McpManagementRepository for PgControlPlaneStore {
    async fn record_mcp_extension_bundle_import(
        &self,
        workspace_id: Uuid,
        extension_installation_id: Uuid,
        actor_user_id: Uuid,
        result_status: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            insert into mcp_extension_bundle_imports (
                workspace_id, extension_installation_id, imported_by, result_status
            ) values ($1, $2, $3, $4)
            on conflict (workspace_id, extension_installation_id)
            do update set
                imported_by = excluded.imported_by,
                result_status = excluded.result_status,
                imported_at = now()
            "#,
        )
        .bind(workspace_id)
        .bind(extension_installation_id)
        .bind(actor_user_id)
        .bind(result_status)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn has_mcp_extension_bundle_import(
        &self,
        workspace_id: Uuid,
        extension_installation_id: Uuid,
    ) -> Result<bool> {
        Ok(sqlx::query_scalar(
            r#"
            select exists(
                select 1
                from mcp_extension_bundle_imports
                where workspace_id = $1 and extension_installation_id = $2
            )
            "#,
        )
        .bind(workspace_id)
        .bind(extension_installation_id)
        .fetch_one(self.pool())
        .await?)
    }

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

    async fn get_mcp_client_credential(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        instance_record_id: Uuid,
        master_key: &str,
    ) -> Result<Option<String>> {
        self.get_mcp_client_credential_impl(user_id, workspace_id, instance_record_id, master_key)
            .await
    }

    async fn upsert_mcp_client_credential(
        &self,
        input: &UpsertMcpClientCredentialInput,
    ) -> Result<()> {
        self.upsert_mcp_client_credential_impl(input).await
    }

    async fn delete_mcp_client_credential(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        instance_record_id: Uuid,
    ) -> Result<()> {
        self.delete_mcp_client_credential_impl(user_id, workspace_id, instance_record_id)
            .await
    }

    async fn list_mcp_upstream_connections(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::McpUpstreamConnectionRecord>> {
        self.list_mcp_upstream_connections_impl(workspace_id).await
    }

    async fn get_mcp_upstream_connection(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> Result<Option<domain::McpUpstreamConnectionRecord>> {
        self.get_mcp_upstream_connection_impl(workspace_id, connection_id)
            .await
    }

    async fn create_mcp_upstream_connection(
        &self,
        input: &CreateMcpUpstreamConnectionInput,
    ) -> Result<domain::McpUpstreamConnectionRecord> {
        self.create_mcp_upstream_connection_impl(input).await
    }

    async fn update_mcp_upstream_connection(
        &self,
        input: &UpdateMcpUpstreamConnectionInput,
    ) -> Result<domain::McpUpstreamConnectionRecord> {
        self.update_mcp_upstream_connection_impl(input).await
    }

    async fn delete_mcp_upstream_connection(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> Result<()> {
        self.delete_mcp_upstream_connection_impl(workspace_id, connection_id)
            .await
    }

    async fn get_mcp_upstream_secret(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        master_key: &str,
    ) -> Result<Option<serde_json::Value>> {
        self.get_mcp_upstream_secret_impl(workspace_id, connection_id, master_key)
            .await
    }

    async fn upsert_mcp_upstream_secret(&self, input: &UpsertMcpUpstreamSecretInput) -> Result<()> {
        self.upsert_mcp_upstream_secret_impl(input).await
    }

    async fn delete_mcp_upstream_secret(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> Result<()> {
        self.delete_mcp_upstream_secret_impl(workspace_id, connection_id)
            .await
    }

    async fn record_mcp_upstream_connection_result(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        connected_at: Option<time::OffsetDateTime>,
        discovered_at: Option<time::OffsetDateTime>,
        last_error: Option<&str>,
    ) -> Result<()> {
        self.record_mcp_upstream_connection_result_impl(
            workspace_id,
            connection_id,
            connected_at,
            discovered_at,
            last_error,
        )
        .await
    }

    async fn list_mcp_upstream_tool_sources(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> Result<Vec<domain::McpUpstreamToolSourceRecord>> {
        self.list_mcp_upstream_tool_sources_impl(workspace_id, connection_id)
            .await
    }

    async fn upsert_mcp_upstream_tool_source(
        &self,
        input: &UpsertMcpUpstreamToolSourceInput,
    ) -> Result<domain::McpUpstreamToolSourceRecord> {
        self.upsert_mcp_upstream_tool_source_impl(input).await
    }

    async fn link_mcp_upstream_tool_source(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        remote_tool_name: &str,
        tool_record_id: Uuid,
    ) -> Result<domain::McpUpstreamToolSourceRecord> {
        self.link_mcp_upstream_tool_source_impl(
            workspace_id,
            connection_id,
            remote_tool_name,
            tool_record_id,
        )
        .await
    }

    async fn mark_mcp_upstream_tool_sources_missing(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        discovered_remote_tool_names: &[String],
    ) -> Result<()> {
        self.mark_mcp_upstream_tool_sources_missing_impl(
            workspace_id,
            connection_id,
            discovered_remote_tool_names,
        )
        .await
    }

    async fn create_mcp_instance(
        &self,
        input: &CreateMcpInstanceInput,
    ) -> Result<domain::McpInstanceRecord> {
        let mut transaction = self.pool().begin().await?;
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
                webmcp_exposure,
                created_by,
                updated_by
            ) values (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $9
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
        .bind(input.webmcp_exposure.as_str())
        .bind(input.actor_user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_mcp_instance_insert_error)?;

        sqlx::query(
            r#"
            insert into mcp_instance_discovery_policies (
                id,
                workspace_id,
                instance_record_id,
                created_by,
                updated_by
            ) values ($1, $2, $3, $4, $4)
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(input.id)
        .bind(input.actor_user_id)
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        map_instance(row)
    }

    async fn create_mcp_instance_graph_atomically(
        &self,
        input: &CreateMcpInstanceGraphInput,
    ) -> Result<domain::McpInstanceRecord> {
        let instance = &input.instance;
        let mut transaction = self.pool().begin().await?;
        let row = sqlx::query(
            r#"
            insert into mcp_instances (
                id, workspace_id, instance_id, name, description_short, status,
                default_entry_path, webmcp_exposure, created_by, updated_by
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
            returning *
            "#,
        )
        .bind(instance.id)
        .bind(instance.workspace_id)
        .bind(&instance.instance_id)
        .bind(&instance.name)
        .bind(&instance.description_short)
        .bind(instance.status.as_str())
        .bind(&instance.default_entry_path)
        .bind(instance.webmcp_exposure.as_str())
        .bind(instance.actor_user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_mcp_instance_insert_error)?;

        for group in &input.groups {
            sqlx::query(
                r#"
                insert into mcp_groups (
                    id, instance_record_id, path, display_name, description_short,
                    enabled, sort_order, scope_id, created_by, updated_by
                ) values (
                    $1, $2, $3, $4, $5, $6, $7,
                    (select scope_id from mcp_instances where id = $2), $8, $8
                )
                "#,
            )
            .bind(group.id)
            .bind(group.instance_record_id)
            .bind(&group.path)
            .bind(&group.display_name)
            .bind(&group.description_short)
            .bind(group.enabled)
            .bind(group.sort_order)
            .bind(group.actor_user_id)
            .execute(&mut *transaction)
            .await?;
        }

        for binding in &input.bindings {
            sqlx::query(
                r#"
                insert into mcp_tool_bindings (
                    id, instance_record_id, tool_record_id, group_path, display_alias,
                    visible, sort_order, scope_id, created_by, updated_by
                ) values (
                    $1, $2, $3, $4, $5, $6, $7,
                    (select scope_id from mcp_instances where id = $2), $8, $8
                )
                "#,
            )
            .bind(binding.id)
            .bind(binding.instance_record_id)
            .bind(binding.tool_record_id)
            .bind(&binding.group_path)
            .bind(&binding.display_alias)
            .bind(binding.visible)
            .bind(binding.sort_order)
            .bind(binding.actor_user_id)
            .execute(&mut *transaction)
            .await?;
        }

        let policy = &input.discovery_policy;
        sqlx::query(
            r#"
            insert into mcp_instance_discovery_policies (
                id, workspace_id, instance_record_id, list_default_limit,
                list_max_depth, list_regex_enabled, list_regex_max_length,
                list_return_fields, created_by, updated_by
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(policy.workspace_id)
        .bind(policy.instance_record_id)
        .bind(policy.list_default_limit)
        .bind(policy.list_max_depth)
        .bind(policy.list_regex_enabled)
        .bind(policy.list_regex_max_length)
        .bind(&policy.list_return_fields)
        .bind(policy.actor_user_id)
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        map_instance(row)
    }

    async fn replace_mcp_bundle_graph_atomically(
        &self,
        input: &ReplaceMcpBundleGraphInput,
    ) -> Result<()> {
        bundle_graph::replace_mcp_bundle_graph_atomically(self, input).await
    }

    async fn seed_mcp_bundle_graph_once_atomically(
        &self,
        input: &control_plane_contracts::ports::SeedMcpBundleGraphInput,
    ) -> Result<()> {
        managed_bundle_graph::seed_mcp_bundle_graph_once_atomically(self, input).await
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
                webmcp_exposure = $7,
                updated_by = $8,
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
        .bind(input.webmcp_exposure.as_str())
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

    async fn move_mcp_group(
        &self,
        actor_user_id: Uuid,
        instance_record_id: Uuid,
        source_path: &str,
        target_path: &str,
        sort_order: i32,
    ) -> Result<domain::McpGroupRecord> {
        let mut transaction = self.pool().begin().await?;
        sqlx::query(
            r#"
            update mcp_tool_bindings
            set group_path = $2 || substring(group_path from char_length($1) + 1),
                updated_by = $4,
                updated_at = now()
            where instance_record_id = $3
              and (group_path = $1 or group_path like $1 || '/%')
            "#,
        )
        .bind(source_path)
        .bind(target_path)
        .bind(instance_record_id)
        .bind(actor_user_id)
        .execute(&mut *transaction)
        .await?;

        let rows = sqlx::query(
            r#"
            update mcp_groups
            set path = $2 || substring(path from char_length($1) + 1),
                sort_order = case when path = $1 then $4 else sort_order end,
                updated_by = $5,
                updated_at = now()
            where instance_record_id = $3
              and (path = $1 or path like $1 || '/%')
            returning *
            "#,
        )
        .bind(source_path)
        .bind(target_path)
        .bind(instance_record_id)
        .bind(sort_order)
        .bind(actor_user_id)
        .fetch_all(&mut *transaction)
        .await?;
        let row = rows
            .into_iter()
            .find(|row| row.get::<String, _>("path") == target_path)
            .ok_or(ControlPlaneError::NotFound("mcp_group"))?;
        transaction.commit().await?;
        map_group(row)
    }

    async fn delete_mcp_group_subtree(&self, instance_record_id: Uuid, path: &str) -> Result<()> {
        let mut transaction = self.pool().begin().await?;
        let deleted_bindings = sqlx::query(
            r#"
            delete from mcp_tool_bindings
            where instance_record_id = $1
              and (
                group_path = $2
                or starts_with(group_path, case when $2 = '/' then '/' else $2 || '/' end)
              )
            "#,
        )
        .bind(instance_record_id)
        .bind(path)
        .execute(&mut *transaction)
        .await?;
        let deleted_groups = sqlx::query(
            r#"
            delete from mcp_groups
            where instance_record_id = $1
              and (
                path = $2
                or starts_with(path, case when $2 = '/' then '/' else $2 || '/' end)
              )
            "#,
        )
        .bind(instance_record_id)
        .bind(path)
        .execute(&mut *transaction)
        .await?;
        if deleted_bindings.rows_affected() == 0 && deleted_groups.rows_affected() == 0 {
            return Err(ControlPlaneError::NotFound("mcp_group").into());
        }
        transaction.commit().await?;
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
                full_description,
                interface_id,
                execution_kind,
                upstream_connection_id,
                remote_tool_name,
                source_schema_hash,
                assistant_client_capability_code,
                parameter_schema,
                result_schema,
                input_mapping,
                output_mapping,
                permission_code,
                risk_level,
                des_id,
                des_id_required,
                status,
                created_by,
                updated_by,
                max_inline_chars,
                response_fields
            ) values (
                $1, $2, $3, $4, $5, $6, $7, $8,
                $9, $10, $11, $12, $13, $14, $15, $16,
                $17, $18, $19, $20, $21, $22, $22, $23, $24
            )
            returning *
            "#,
        )
        .bind(input.id)
        .bind(input.workspace_id)
        .bind(&input.tool_id)
        .bind(&input.name)
        .bind(&input.short_description)
        .bind(&input.full_description)
        .bind(input.execution_target.interface_id())
        .bind(execution_target_kind(&input.execution_target))
        .bind(execution_target_upstream_connection_id(
            &input.execution_target,
        ))
        .bind(execution_target_remote_tool_name(&input.execution_target))
        .bind(execution_target_source_schema_hash(&input.execution_target))
        .bind(execution_target_assistant_client_capability_code(
            &input.execution_target,
        ))
        .bind(&input.parameter_schema)
        .bind(&input.result_schema)
        .bind(&input.input_mapping)
        .bind(&input.output_mapping)
        .bind(&input.permission_code)
        .bind(input.risk_level.as_str())
        .bind(&input.des_id)
        .bind(input.des_id_required)
        .bind(input.status.as_str())
        .bind(input.actor_user_id)
        .bind(input.max_inline_chars)
        .bind(
            input
                .response_fields
                .as_ref()
                .map(serde_json::to_value)
                .transpose()?,
        )
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
                full_description = $5,
                interface_id = $6,
                execution_kind = $7,
                upstream_connection_id = $8,
                remote_tool_name = $9,
                source_schema_hash = $10,
                assistant_client_capability_code = $11,
                parameter_schema = $12,
                result_schema = $13,
                input_mapping = $14,
                output_mapping = $15,
                permission_code = $16,
                risk_level = $17,
                des_id = $18,
                des_id_required = $19,
                status = $20,
                revision = revision + 1,
                updated_by = $21,
                updated_at = now(),
                max_inline_chars = $22,
                response_fields = $23
            where workspace_id = $1 and tool_id = $2
            returning *
            "#,
        )
        .bind(input.workspace_id)
        .bind(&input.tool_id)
        .bind(&input.name)
        .bind(&input.short_description)
        .bind(&input.full_description)
        .bind(input.execution_target.interface_id())
        .bind(execution_target_kind(&input.execution_target))
        .bind(execution_target_upstream_connection_id(
            &input.execution_target,
        ))
        .bind(execution_target_remote_tool_name(&input.execution_target))
        .bind(execution_target_source_schema_hash(&input.execution_target))
        .bind(execution_target_assistant_client_capability_code(
            &input.execution_target,
        ))
        .bind(&input.parameter_schema)
        .bind(&input.result_schema)
        .bind(&input.input_mapping)
        .bind(&input.output_mapping)
        .bind(&input.permission_code)
        .bind(input.risk_level.as_str())
        .bind(&input.des_id)
        .bind(input.des_id_required)
        .bind(input.status.as_str())
        .bind(input.actor_user_id)
        .bind(input.max_inline_chars)
        .bind(
            input
                .response_fields
                .as_ref()
                .map(serde_json::to_value)
                .transpose()?,
        )
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

    async fn list_mcp_instance_discovery_policies(
        &self,
        instance_record_ids: &[Uuid],
    ) -> Result<Vec<domain::McpInstanceDiscoveryPolicyRecord>> {
        if instance_record_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            r#"
            select *
            from mcp_instance_discovery_policies
            where instance_record_id = any($1)
            order by instance_record_id asc
            "#,
        )
        .bind(instance_record_ids)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter()
            .map(map_instance_discovery_policy)
            .collect()
    }

    async fn get_mcp_instance_discovery_policy(
        &self,
        instance_record_id: Uuid,
    ) -> Result<Option<domain::McpInstanceDiscoveryPolicyRecord>> {
        let row = sqlx::query(
            r#"
            select *
            from mcp_instance_discovery_policies
            where instance_record_id = $1
            "#,
        )
        .bind(instance_record_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_instance_discovery_policy).transpose()
    }

    async fn update_mcp_instance_discovery_policy(
        &self,
        input: &UpdateMcpInstanceDiscoveryPolicyInput,
    ) -> Result<domain::McpInstanceDiscoveryPolicyRecord> {
        let row = sqlx::query(
            r#"
            update mcp_instance_discovery_policies
            set
                list_default_limit = $3,
                list_max_depth = $4,
                list_regex_enabled = $5,
                list_regex_max_length = $6,
                list_return_fields = $7,
                updated_by = $8,
                updated_at = now()
            where workspace_id = $1 and instance_record_id = $2
            returning *
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.instance_record_id)
        .bind(input.list_default_limit)
        .bind(input.list_max_depth)
        .bind(input.list_regex_enabled)
        .bind(input.list_regex_max_length)
        .bind(&input.list_return_fields)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_instance_discovery_policy(row)
    }
}

#[path = "mcp_management_repository/upstream_connections.rs"]
mod upstream_connections;
