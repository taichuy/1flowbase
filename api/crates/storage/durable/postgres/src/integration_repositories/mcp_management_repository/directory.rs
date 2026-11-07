use super::*;
use control_plane_contracts::ports::ReplaceMcpInstanceDirectoryInput;
use domain::mcp_management::McpInstanceDirectory;
use sqlx::{PgConnection, Postgres, Transaction};
use std::collections::{HashMap, HashSet};

impl PgControlPlaneStore {
    pub(super) async fn get_mcp_instance_directory_impl(
        &self,
        workspace_id: Uuid,
        instance_id: &str,
    ) -> Result<McpInstanceDirectory> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("set transaction isolation level repeatable read, read only")
            .execute(&mut *tx)
            .await?;
        let id = directory_instance_id(&mut tx, workspace_id, instance_id, false).await?;
        let (groups, bindings) = read_directory_records(&mut tx, id).await?;
        let snapshot = McpInstanceDirectory::from_records(id, &groups, &bindings);
        tx.commit().await?;
        Ok(snapshot)
    }

    pub(super) async fn replace_mcp_instance_directory_impl(
        &self,
        input: &ReplaceMcpInstanceDirectoryInput,
    ) -> Result<McpInstanceDirectory> {
        self.replace_directory_transaction(input)
            .await
            .map_err(directory_error)
    }

    async fn replace_directory_transaction(
        &self,
        input: &ReplaceMcpInstanceDirectoryInput,
    ) -> Result<McpInstanceDirectory> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("set transaction isolation level read committed")
            .execute(&mut *tx)
            .await?;
        // Existing writers do not lock the instance. Table locks also serialize those writers.
        // Keep this order for every directory replacement, including empty directories.
        sqlx::query("lock table mcp_groups in share row exclusive mode")
            .execute(&mut *tx)
            .await?;
        sqlx::query("lock table mcp_tool_bindings in share row exclusive mode")
            .execute(&mut *tx)
            .await?;
        let id =
            directory_instance_id(&mut tx, input.workspace_id, &input.instance_id, true).await?;
        let (old_groups, old_bindings) = read_directory_records(&mut tx, id).await?;
        let current = McpInstanceDirectory::from_records(id, &old_groups, &old_bindings);
        let mut expected = input.expected_snapshot.clone();
        expected.canonicalize();
        if current != expected {
            return Err(ControlPlaneError::Conflict("mcp_directory_snapshot").into());
        }
        let groups_by_id = old_groups
            .iter()
            .map(|group| (group.id, group))
            .collect::<HashMap<_, _>>();
        let group_paths = old_groups
            .iter()
            .map(|group| group.path.as_str())
            .collect::<HashSet<_>>();
        let bindings_by_id = old_bindings
            .iter()
            .map(|binding| (binding.id, binding))
            .collect::<HashMap<_, _>>();
        let binding_identities = old_bindings
            .iter()
            .map(|binding| (binding.group_path.as_str(), binding.tool_record_id))
            .collect::<HashSet<_>>();
        // Validate IDs against the locked current graph. Omitting an ID cannot silently
        // delete/recreate a record with the same directory identity.
        for group in &input.groups {
            match group.id {
                Some(group_id) if !groups_by_id.contains_key(&group_id) => {
                    return Err(ControlPlaneError::InvalidInput("group_id").into())
                }
                None if group_paths.contains(group.path.as_str()) => {
                    return Err(ControlPlaneError::Conflict("mcp_directory_group_identity").into())
                }
                _ => {}
            }
        }
        let tool_names = input
            .bindings
            .iter()
            .map(|binding| binding.tool_id.clone())
            .collect::<Vec<_>>();
        let tools = sqlx::query("select id,tool_id from mcp_tools where workspace_id=$1 and tool_id=any($2) order by id for key share")
            .bind(input.workspace_id).bind(&tool_names).fetch_all(&mut *tx).await?
            .into_iter().map(|row| (row.get::<String, _>("tool_id"), row.get::<Uuid, _>("id")))
            .collect::<HashMap<_, _>>();
        let mut tool_ids = Vec::with_capacity(input.bindings.len());
        for binding in &input.bindings {
            let tool_record_id = *tools
                .get(&binding.tool_id)
                .ok_or(ControlPlaneError::NotFound("mcp_tool"))?;
            match binding.id {
                Some(binding_id) => {
                    let old = bindings_by_id
                        .get(&binding_id)
                        .ok_or(ControlPlaneError::InvalidInput("binding_id"))?;
                    if old.tool_record_id != tool_record_id {
                        return Err(
                            ControlPlaneError::Conflict("mcp_directory_binding_identity").into(),
                        );
                    }
                }
                None if binding_identities
                    .contains(&(binding.group_path.as_str(), tool_record_id)) =>
                {
                    return Err(
                        ControlPlaneError::Conflict("mcp_directory_binding_identity").into(),
                    )
                }
                None => {}
            }
            tool_ids.push(tool_record_id);
        }
        // Reinsert inside the transaction to support path swaps without transient unique
        // conflicts. Existing creation identity and audit values are copied verbatim.
        sqlx::query("delete from mcp_tool_bindings where instance_record_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("delete from mcp_groups where instance_record_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let now = time::OffsetDateTime::now_utc();
        for group in &input.groups {
            let old = group.id.and_then(|group_id| groups_by_id.get(&group_id));
            sqlx::query("insert into mcp_groups (id,instance_record_id,path,display_name,description_short,enabled,sort_order,created_by,created_at,updated_by,updated_at) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
                .bind(group.id.unwrap_or_else(Uuid::now_v7)).bind(id).bind(&group.path)
                .bind(&group.display_name).bind(&group.description_short).bind(group.enabled).bind(group.sort_order)
                .bind(old.map_or(input.actor_user_id, |old| old.created_by))
                .bind(old.map_or(now, |old| old.created_at)).bind(input.actor_user_id).bind(now)
                .execute(&mut *tx).await?;
        }
        for (binding, tool_record_id) in input.bindings.iter().zip(tool_ids) {
            let old = binding
                .id
                .and_then(|binding_id| bindings_by_id.get(&binding_id));
            sqlx::query("insert into mcp_tool_bindings (id,instance_record_id,tool_record_id,group_path,display_alias,visible,sort_order,created_by,created_at,updated_by,updated_at) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
                .bind(binding.id.unwrap_or_else(Uuid::now_v7)).bind(id).bind(tool_record_id)
                .bind(&binding.group_path).bind(&binding.display_alias).bind(binding.visible).bind(binding.sort_order)
                .bind(old.map_or(input.actor_user_id, |old| old.created_by))
                .bind(old.map_or(now, |old| old.created_at)).bind(input.actor_user_id).bind(now)
                .execute(&mut *tx).await?;
        }
        let (groups, bindings) = read_directory_records(&mut tx, id).await?;
        let snapshot = McpInstanceDirectory::from_records(id, &groups, &bindings);
        tx.commit().await?;
        Ok(snapshot)
    }
}

async fn directory_instance_id(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    instance_id: &str,
    lock: bool,
) -> Result<Uuid> {
    let query = if lock {
        "select id from mcp_instances where workspace_id=$1 and instance_id=$2 for key share"
    } else {
        "select id from mcp_instances where workspace_id=$1 and instance_id=$2"
    };
    sqlx::query_scalar(query)
        .bind(workspace_id)
        .bind(instance_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| ControlPlaneError::NotFound("mcp_instance").into())
}

async fn read_directory_records(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<(
    Vec<domain::McpGroupRecord>,
    Vec<domain::McpToolBindingRecord>,
)> {
    let groups = sqlx::query("select * from mcp_groups where instance_record_id=$1 order by id")
        .bind(id)
        .fetch_all(&mut *connection)
        .await?
        .into_iter()
        .map(map_group)
        .collect::<Result<Vec<_>>>()?;
    let bindings = sqlx::query("select b.*,t.tool_id from mcp_tool_bindings b join mcp_tools t on t.id=b.tool_record_id where b.instance_record_id=$1 order by b.id")
        .bind(id).fetch_all(connection).await?.into_iter().map(map_binding).collect::<Result<Vec<_>>>()?;
    Ok((groups, bindings))
}

fn directory_error(error: anyhow::Error) -> anyhow::Error {
    if let Some(sqlx::Error::Database(database)) = error.downcast_ref::<sqlx::Error>() {
        if matches!(
            database.code().as_deref(),
            Some("23505" | "40001" | "40P01")
        ) {
            return ControlPlaneError::Conflict("mcp_directory_concurrent_write").into();
        }
    }
    error
}
