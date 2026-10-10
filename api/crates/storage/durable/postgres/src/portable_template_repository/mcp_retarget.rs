//! Narrow binding identity CAS; invoked only inside the template native-owner batch.
use super::*;

pub(super) async fn retarget(
    connection: &mut sqlx::PgConnection,
    scope: &TemplateBaselineScope,
    command: &TemplateMcpBindingRetarget,
) -> Result<bool> {
    anyhow::ensure!(
        command.key.kind == "mcp_binding",
        "template_mcp_retarget_kind"
    );
    let old: [String; 3] = serde_json::from_str(&command.expected_target_id)?;
    let new: [String; 3] = serde_json::from_str(&command.intent.target_id)?;
    anyhow::ensure!(
        old[..2] == new[..2] && old[2] != new[2],
        "template_mcp_retarget_identity"
    );
    let expected = command
        .intent
        .expected_fingerprint
        .as_deref()
        .context("template_mcp_retarget_preimage_required")?;
    let row = sqlx::query(
        "SELECT binding.id, binding.tool_record_id, binding.display_alias, binding.visible, binding.sort_order,
                instance.id as instance_record_id
         FROM mcp_tool_bindings binding
         JOIN mcp_instances instance ON instance.id=binding.instance_record_id
         JOIN mcp_tools tool ON tool.id=binding.tool_record_id
         WHERE instance.workspace_id=$1 AND instance.instance_id=$2 AND binding.group_path=$3
           AND tool.workspace_id=$1 AND tool.tool_id=$4
         FOR UPDATE OF binding"
    ).bind(scope.workspace_id).bind(&old[0]).bind(&old[1]).bind(&old[2])
        .fetch_optional(&mut *connection).await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let value = serde_json::json!({
        "display_alias": row.try_get::<Option<String>,_>("display_alias")?,
        "visible": row.try_get::<bool,_>("visible")?,
        "sort_order": row.try_get::<i32,_>("sort_order")?,
    });
    if template_resource_fingerprint(&value)? != expected {
        return Ok(false);
    }
    let binding_id: Uuid = row.try_get("id")?;
    let old_tool: Uuid = row.try_get("tool_record_id")?;
    let instance_id: Uuid = row.try_get("instance_record_id")?;
    let new_tool: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM mcp_tools WHERE workspace_id=$1 AND tool_id=$2")
            .bind(scope.workspace_id)
            .bind(&new[2])
            .fetch_optional(&mut *connection)
            .await?;
    let Some(new_tool) = new_tool else {
        return Ok(false);
    };
    let occupied: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM mcp_tool_bindings WHERE instance_record_id=$1 AND group_path=$2 AND tool_record_id=$3)")
        .bind(instance_id).bind(&new[1]).bind(new_tool).fetch_one(&mut *connection).await?;
    if occupied {
        return Ok(false);
    }
    let moved = sqlx::query(
        "UPDATE application_template_resource_baselines
         SET target_id=$6, pending_operation_id=$7, pending_expected_fingerprint=$8,
             pending_desired_fingerprint=$9, generation=generation+1, updated_at=now()
         WHERE workspace_id=$1 AND template_id=$2 AND kind='mcp_binding' AND source_id=$3
           AND target_id=$4 AND generation=$5 AND applied_fingerprint=$8
           AND pending_operation_id IS NULL AND committed_operation_id IS NULL",
    )
    .bind(scope.workspace_id)
    .bind(&scope.template_id)
    .bind(&command.key.source_id)
    .bind(&command.expected_target_id)
    .bind(command.expected_generation)
    .bind(&command.intent.target_id)
    .bind(command.intent.operation_id)
    .bind(expected)
    .bind(&command.intent.desired_fingerprint)
    .execute(&mut *connection)
    .await?;
    if moved.rows_affected() != 1 {
        return Ok(false);
    }
    let changed = sqlx::query("UPDATE mcp_tool_bindings SET tool_record_id=$2, updated_by=$3, updated_at=now() WHERE id=$1 AND tool_record_id=$4")
        .bind(binding_id).bind(new_tool).bind(command.actor_user_id).bind(old_tool)
        .execute(&mut *connection).await?;
    anyhow::ensure!(
        changed.rows_affected() == 1,
        "template_mcp_retarget_native_conflict"
    );
    Ok(true)
}
