use anyhow::Result;
use control_plane::errors::ControlPlaneError;
use control_plane::ports::ReplaceMcpBundleGraphInput;
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

use super::{
    execution_target_kind, execution_target_remote_tool_name, execution_target_source_schema_hash,
    execution_target_upstream_connection_id,
};

pub(super) async fn replace_mcp_bundle_graph_atomically(
    store: &PgControlPlaneStore,
    input: &ReplaceMcpBundleGraphInput,
) -> Result<()> {
    let mut transaction = store.pool().begin().await?;

    for connection in &input.connections {
        sqlx::query(
            r#"
                insert into mcp_upstream_connections (
                    id, workspace_id, name, endpoint, transport, auth_type,
                    custom_header_name, status, created_by, updated_by
                ) values ($1,$2,$3,$4,$5,$6,$7,'disabled',$8,$8)
                on conflict (id) do update set
                    name=excluded.name,
                    endpoint=excluded.endpoint,
                    transport=excluded.transport,
                    auth_type=excluded.auth_type,
                    custom_header_name=excluded.custom_header_name,
                    status=case
                        when exists (
                            select 1 from mcp_upstream_connection_secrets secret
                            where secret.upstream_connection_id=excluded.id
                        ) then $9
                        else 'disabled'
                    end,
                    updated_by=excluded.updated_by,
                    updated_at=now()
                where mcp_upstream_connections.workspace_id=excluded.workspace_id
                "#,
        )
        .bind(connection.connection_id)
        .bind(input.workspace_id)
        .bind(&connection.name)
        .bind(&connection.endpoint)
        .bind(connection.transport.as_str())
        .bind(connection.auth_type.as_str())
        .bind(&connection.custom_header_name)
        .bind(input.actor_user_id)
        .bind(connection.status.as_str())
        .execute(&mut *transaction)
        .await?;
    }

    for tool in &input.tools {
        sqlx::query(
            r#"
                insert into mcp_tools (
                    id, workspace_id, tool_id, name, short_description, full_description,
                    interface_id, execution_kind, upstream_connection_id, remote_tool_name,
                    source_schema_hash, parameter_schema, result_schema, input_mapping,
                    output_mapping, permission_code, risk_level, des_id, des_id_required,
                    status, created_by, updated_by
                ) values (
                    $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$21
                )
                on conflict (workspace_id, tool_id) do update set
                    name=excluded.name,
                    short_description=excluded.short_description,
                    full_description=excluded.full_description,
                    interface_id=excluded.interface_id,
                    execution_kind=excluded.execution_kind,
                    upstream_connection_id=excluded.upstream_connection_id,
                    remote_tool_name=excluded.remote_tool_name,
                    source_schema_hash=excluded.source_schema_hash,
                    parameter_schema=excluded.parameter_schema,
                    result_schema=excluded.result_schema,
                    input_mapping=excluded.input_mapping,
                    output_mapping=excluded.output_mapping,
                    permission_code=excluded.permission_code,
                    risk_level=excluded.risk_level,
                    des_id_required=excluded.des_id_required,
                    status=excluded.status,
                    revision=mcp_tools.revision + 1,
                    updated_by=excluded.updated_by,
                    updated_at=now()
                "#,
        )
        .bind(tool.id)
        .bind(tool.workspace_id)
        .bind(&tool.tool_id)
        .bind(&tool.name)
        .bind(&tool.short_description)
        .bind(&tool.full_description)
        .bind(tool.execution_target.interface_id())
        .bind(execution_target_kind(&tool.execution_target))
        .bind(execution_target_upstream_connection_id(
            &tool.execution_target,
        ))
        .bind(execution_target_remote_tool_name(&tool.execution_target))
        .bind(execution_target_source_schema_hash(&tool.execution_target))
        .bind(&tool.parameter_schema)
        .bind(&tool.result_schema)
        .bind(&tool.input_mapping)
        .bind(&tool.output_mapping)
        .bind(&tool.permission_code)
        .bind(tool.risk_level.as_str())
        .bind(&tool.des_id)
        .bind(tool.des_id_required)
        .bind(tool.status.as_str())
        .bind(tool.actor_user_id)
        .execute(&mut *transaction)
        .await?;
    }

    for instance in &input.instances {
        let instance_record_id: Uuid = sqlx::query_scalar(
            r#"
                insert into mcp_instances (
                    id, workspace_id, instance_id, name, description_short, status,
                    default_entry_path, created_by, updated_by
                ) values ($1,$2,$3,$4,$5,$6,$7,$8,$8)
                on conflict (workspace_id, instance_id) do update set
                    name=excluded.name,
                    description_short=excluded.description_short,
                    status=excluded.status,
                    default_entry_path=excluded.default_entry_path,
                    updated_by=excluded.updated_by,
                    updated_at=now()
                returning id
                "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(&instance.instance_id)
        .bind(&instance.name)
        .bind(&instance.description_short)
        .bind(instance.status.as_str())
        .bind(&instance.default_entry_path)
        .bind(input.actor_user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_mcp_instance_insert_error)?;

        sqlx::query("delete from mcp_tool_bindings where instance_record_id=$1")
            .bind(instance_record_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("delete from mcp_groups where instance_record_id=$1")
            .bind(instance_record_id)
            .execute(&mut *transaction)
            .await?;

        for group in &instance.groups {
            sqlx::query(
                r#"
                    insert into mcp_groups (
                        id, instance_record_id, path, display_name, description_short,
                        enabled, sort_order, scope_id, created_by, updated_by
                    ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$9)
                    "#,
            )
            .bind(Uuid::now_v7())
            .bind(instance_record_id)
            .bind(&group.path)
            .bind(&group.display_name)
            .bind(&group.description_short)
            .bind(group.enabled)
            .bind(group.sort_order)
            .bind(input.workspace_id)
            .bind(input.actor_user_id)
            .execute(&mut *transaction)
            .await?;
        }

        for binding in &instance.bindings {
            let inserted = sqlx::query(
                r#"
                    insert into mcp_tool_bindings (
                        id, instance_record_id, tool_record_id, group_path, display_alias,
                        visible, sort_order, scope_id, created_by, updated_by
                    ) select $1,$2,tool.id,$3,$4,$5,$6,$7,$8,$8
                      from mcp_tools tool
                     where tool.workspace_id=$7 and tool.tool_id=$9
                    "#,
            )
            .bind(Uuid::now_v7())
            .bind(instance_record_id)
            .bind(&binding.group_path)
            .bind(&binding.display_alias)
            .bind(binding.visible)
            .bind(binding.sort_order)
            .bind(input.workspace_id)
            .bind(input.actor_user_id)
            .bind(&binding.tool_id)
            .execute(&mut *transaction)
            .await?;
            if inserted.rows_affected() != 1 {
                return Err(ControlPlaneError::NotFound("mcp_tool").into());
            }
        }

        let policy = &instance.discovery_policy;
        sqlx::query(
            r#"
                insert into mcp_instance_discovery_policies (
                    id, workspace_id, instance_record_id, list_default_limit, list_max_depth,
                    list_regex_enabled, list_regex_max_length, list_return_fields,
                    created_by, updated_by
                ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$9)
                on conflict (instance_record_id) do update set
                    list_default_limit=excluded.list_default_limit,
                    list_max_depth=excluded.list_max_depth,
                    list_regex_enabled=excluded.list_regex_enabled,
                    list_regex_max_length=excluded.list_regex_max_length,
                    list_return_fields=excluded.list_return_fields,
                    updated_by=excluded.updated_by,
                    updated_at=now()
                "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(instance_record_id)
        .bind(policy.list_default_limit)
        .bind(policy.list_max_depth)
        .bind(policy.list_regex_enabled)
        .bind(policy.list_regex_max_length)
        .bind(&policy.list_return_fields)
        .bind(input.actor_user_id)
        .execute(&mut *transaction)
        .await?;
    }

    transaction.commit().await?;
    Ok(())
}
