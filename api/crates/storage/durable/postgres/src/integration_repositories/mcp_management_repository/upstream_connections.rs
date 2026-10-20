use super::*;

impl PgControlPlaneStore {
    pub(super) async fn get_mcp_client_credential_impl(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        instance_record_id: Uuid,
        master_key: &str,
    ) -> Result<Option<String>> {
        let encrypted_secret_json: Option<serde_json::Value> = sqlx::query_scalar(
            r#"
            select encrypted_secret_json
            from mcp_client_credentials
            where user_id = $1 and workspace_id = $2 and instance_record_id = $3
            "#,
        )
        .bind(user_id)
        .bind(workspace_id)
        .bind(instance_record_id)
        .fetch_optional(self.pool())
        .await?;
        let Some(encrypted_secret_json) = encrypted_secret_json else {
            return Ok(None);
        };
        let secret = decrypt_secret_json(&encrypted_secret_json, master_key)?;
        let api_key = secret
            .get("api_key")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("missing MCP client API key"))?;
        Ok(Some(api_key.to_string()))
    }

    pub(super) async fn upsert_mcp_client_credential_impl(
        &self,
        input: &UpsertMcpClientCredentialInput,
    ) -> Result<()> {
        let encrypted_secret_json =
            encrypt_secret_json(&json!({ "api_key": input.api_key }), &input.master_key)?;
        sqlx::query(
            r#"
            insert into mcp_client_credentials (
                id, user_id, workspace_id, instance_record_id, encrypted_secret_json
            ) values ($1, $2, $3, $4, $5)
            on conflict (user_id, workspace_id, instance_record_id) do update
            set encrypted_secret_json = excluded.encrypted_secret_json,
                updated_at = now()
            "#,
        )
        .bind(input.id)
        .bind(input.user_id)
        .bind(input.workspace_id)
        .bind(input.instance_record_id)
        .bind(encrypted_secret_json)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub(super) async fn delete_mcp_client_credential_impl(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        instance_record_id: Uuid,
    ) -> Result<()> {
        sqlx::query(
            r#"
            delete from mcp_client_credentials
            where user_id = $1 and workspace_id = $2 and instance_record_id = $3
            "#,
        )
        .bind(user_id)
        .bind(workspace_id)
        .bind(instance_record_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub(super) async fn list_mcp_upstream_connections_impl(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::McpUpstreamConnectionRecord>> {
        let rows = sqlx::query(
            r#"
            select c.*,
                   exists(select 1 from mcp_upstream_connection_secrets s
                          where s.upstream_connection_id = c.id) as credentials_configured
            from mcp_upstream_connections c
            where c.workspace_id = $1
            order by c.updated_at desc, c.id desc
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(map_upstream_connection).collect()
    }

    pub(super) async fn get_mcp_upstream_connection_impl(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> Result<Option<domain::McpUpstreamConnectionRecord>> {
        let row = sqlx::query(
            r#"
            select c.*,
                   exists(select 1 from mcp_upstream_connection_secrets s
                          where s.upstream_connection_id = c.id) as credentials_configured
            from mcp_upstream_connections c
            where c.workspace_id = $1 and c.id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(connection_id)
        .fetch_optional(self.pool())
        .await?;
        row.map(map_upstream_connection).transpose()
    }

    pub(super) async fn create_mcp_upstream_connection_impl(
        &self,
        input: &CreateMcpUpstreamConnectionInput,
    ) -> Result<domain::McpUpstreamConnectionRecord> {
        sqlx::query(
            r#"
            insert into mcp_upstream_connections (
                id, workspace_id, name, endpoint, transport, auth_type,
                custom_header_name, status, created_by, updated_by
            ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$9)
            "#,
        )
        .bind(input.id)
        .bind(input.workspace_id)
        .bind(&input.name)
        .bind(&input.endpoint)
        .bind(input.transport.as_str())
        .bind(input.auth_type.as_str())
        .bind(&input.custom_header_name)
        .bind(input.status.as_str())
        .bind(input.actor_user_id)
        .execute(self.pool())
        .await?;
        self.get_mcp_upstream_connection(input.workspace_id, input.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("created MCP upstream connection is missing"))
    }

    pub(super) async fn update_mcp_upstream_connection_impl(
        &self,
        input: &UpdateMcpUpstreamConnectionInput,
    ) -> Result<domain::McpUpstreamConnectionRecord> {
        sqlx::query(
            r#"
            update mcp_upstream_connections
            set name=$3, endpoint=$4, transport=$5, auth_type=$6,
                custom_header_name=$7, status=$8, updated_by=$9, updated_at=now()
            where workspace_id=$1 and id=$2
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.id)
        .bind(&input.name)
        .bind(&input.endpoint)
        .bind(input.transport.as_str())
        .bind(input.auth_type.as_str())
        .bind(&input.custom_header_name)
        .bind(input.status.as_str())
        .bind(input.actor_user_id)
        .execute(self.pool())
        .await?;
        self.get_mcp_upstream_connection(input.workspace_id, input.id)
            .await?
            .ok_or_else(|| ControlPlaneError::NotFound("mcp_upstream_connection").into())
    }

    pub(super) async fn delete_mcp_upstream_connection_impl(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> Result<()> {
        sqlx::query("delete from mcp_upstream_connections where workspace_id=$1 and id=$2")
            .bind(workspace_id)
            .bind(connection_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub(super) async fn get_mcp_upstream_secret_impl(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        master_key: &str,
    ) -> Result<Option<serde_json::Value>> {
        let encrypted: Option<serde_json::Value> = sqlx::query_scalar(
            "select encrypted_secret_json from mcp_upstream_connection_secrets where workspace_id=$1 and upstream_connection_id=$2",
        )
        .bind(workspace_id)
        .bind(connection_id)
        .fetch_optional(self.pool())
        .await?;
        encrypted
            .map(|value| decrypt_secret_json(&value, master_key))
            .transpose()
    }

    pub(super) async fn upsert_mcp_upstream_secret_impl(
        &self,
        input: &UpsertMcpUpstreamSecretInput,
    ) -> Result<()> {
        let encrypted = encrypt_secret_json(&input.plaintext_secret_json, &input.master_key)?;
        sqlx::query(
            r#"
            insert into mcp_upstream_connection_secrets (
                upstream_connection_id, workspace_id, encrypted_secret_json, updated_by
            ) values ($1,$2,$3,$4)
            on conflict (upstream_connection_id) do update
            set encrypted_secret_json=excluded.encrypted_secret_json,
                updated_by=excluded.updated_by, updated_at=now()
            "#,
        )
        .bind(input.upstream_connection_id)
        .bind(input.workspace_id)
        .bind(encrypted)
        .bind(input.actor_user_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub(super) async fn delete_mcp_upstream_secret_impl(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> Result<()> {
        sqlx::query("delete from mcp_upstream_connection_secrets where workspace_id=$1 and upstream_connection_id=$2")
            .bind(workspace_id)
            .bind(connection_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub(super) async fn record_mcp_upstream_connection_result_impl(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        connected_at: Option<time::OffsetDateTime>,
        discovered_at: Option<time::OffsetDateTime>,
        last_error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            update mcp_upstream_connections
            set last_connected_at=coalesce($3,last_connected_at),
                last_discovered_at=coalesce($4,last_discovered_at),
                last_error=$5, updated_at=now()
            where workspace_id=$1 and id=$2
            "#,
        )
        .bind(workspace_id)
        .bind(connection_id)
        .bind(connected_at)
        .bind(discovered_at)
        .bind(last_error)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub(super) async fn list_mcp_upstream_tool_sources_impl(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> Result<Vec<domain::McpUpstreamToolSourceRecord>> {
        let rows = sqlx::query(
            r#"
            select s.*, t.tool_id as imported_tool_id
            from mcp_upstream_tool_sources s
            left join mcp_tools t on t.id=s.imported_tool_record_id
            where s.workspace_id=$1 and s.upstream_connection_id=$2
            order by s.remote_tool_name
            "#,
        )
        .bind(workspace_id)
        .bind(connection_id)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(map_upstream_source).collect()
    }

    pub(super) async fn upsert_mcp_upstream_tool_source_impl(
        &self,
        input: &UpsertMcpUpstreamToolSourceInput,
    ) -> Result<domain::McpUpstreamToolSourceRecord> {
        sqlx::query(
            r#"
            insert into mcp_upstream_tool_sources (
                id,workspace_id,upstream_connection_id,remote_tool_name,description,
                input_schema,output_schema,schema_hash,source_status,discovered_at
            ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            on conflict (upstream_connection_id,remote_tool_name) do update
            set description=excluded.description,input_schema=excluded.input_schema,
                output_schema=excluded.output_schema,schema_hash=excluded.schema_hash,
                source_status=case
                    when mcp_upstream_tool_sources.imported_tool_record_id is null then 'not_imported'
                    when mcp_upstream_tool_sources.schema_hash=excluded.schema_hash then 'imported'
                    else 'definition_changed'
                end,
                discovered_at=excluded.discovered_at,updated_at=now()
            "#,
        )
        .bind(input.id)
        .bind(input.workspace_id)
        .bind(input.upstream_connection_id)
        .bind(&input.remote_tool_name)
        .bind(&input.description)
        .bind(&input.input_schema)
        .bind(&input.output_schema)
        .bind(&input.schema_hash)
        .bind(input.source_status.as_str())
        .bind(input.discovered_at)
        .execute(self.pool())
        .await?;
        self.list_mcp_upstream_tool_sources(input.workspace_id, input.upstream_connection_id)
            .await?
            .into_iter()
            .find(|source| source.remote_tool_name == input.remote_tool_name)
            .ok_or_else(|| anyhow::anyhow!("upserted MCP upstream source is missing"))
    }

    pub(super) async fn link_mcp_upstream_tool_source_impl(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        remote_tool_name: &str,
        tool_record_id: Uuid,
    ) -> Result<domain::McpUpstreamToolSourceRecord> {
        sqlx::query(
            r#"
            update mcp_upstream_tool_sources
            set imported_tool_record_id=$4,source_status='imported',updated_at=now()
            where workspace_id=$1 and upstream_connection_id=$2 and remote_tool_name=$3
            "#,
        )
        .bind(workspace_id)
        .bind(connection_id)
        .bind(remote_tool_name)
        .bind(tool_record_id)
        .execute(self.pool())
        .await?;
        self.list_mcp_upstream_tool_sources(workspace_id, connection_id)
            .await?
            .into_iter()
            .find(|source| source.remote_tool_name == remote_tool_name)
            .ok_or_else(|| ControlPlaneError::NotFound("mcp_upstream_tool_source").into())
    }

    pub(super) async fn mark_mcp_upstream_tool_sources_missing_impl(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        discovered_remote_tool_names: &[String],
    ) -> Result<()> {
        sqlx::query(
            r#"
            update mcp_upstream_tool_sources
            set source_status='remote_missing',updated_at=now()
            where workspace_id=$1 and upstream_connection_id=$2
              and not (remote_tool_name = any($3))
            "#,
        )
        .bind(workspace_id)
        .bind(connection_id)
        .bind(discovered_remote_tool_names)
        .execute(self.pool())
        .await?;
        Ok(())
    }
}
