//! Atomic host-owned record policy storage. Physical resource scope is deliberately absent.
use crate::repositories::PgControlPlaneStore;
use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::{
    ports::RecordPermissionRepository, ControlPlaneContractError as Error,
};
use domain::{
    ActorContext, RecordPermissionGrant, RecordPermissionKey, RecordPermissionMode,
    RecordPermissionPolicy, RecordPermissionResourceKey, RecordPermissionRole,
    ReplaceRecordPermissions,
};
use sqlx::{Postgres, Row, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

fn policy_mode(mode: &str) -> Result<RecordPermissionMode> {
    match mode {
        "inherit" => Ok(RecordPermissionMode::Inherit),
        "restricted" => Ok(RecordPermissionMode::Restricted),
        _ => Err(Error::InvalidInput("record_permission_mode").into()),
    }
}

async fn read_policy(
    transaction: &mut Transaction<'_, Postgres>,
    key: &RecordPermissionKey,
) -> Result<Option<RecordPermissionPolicy>> {
    let row = sqlx::query("select id, mode, revision from plugin_record_permission_policies where workspace_id=$1 and publisher_namespace=$2 and plugin_code=$3 and resource_code=$4 and record_id=$5")
        .bind(key.workspace_id).bind(&key.resource.publisher_namespace).bind(&key.resource.plugin_code)
        .bind(&key.resource.resource_code).bind(&key.record_id).fetch_optional(&mut **transaction).await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let policy_id: Uuid = row.try_get("id")?;
    let mode = policy_mode(row.try_get("mode")?)?;
    let grants = sqlx::query("select role_id, actions from plugin_record_permission_grants where policy_id=$1 order by role_id")
        .bind(policy_id).fetch_all(&mut **transaction).await?.into_iter().map(|row| Ok(RecordPermissionGrant {
            role_id: row.try_get("role_id")?, actions: row.try_get("actions")?,
        })).collect::<Result<Vec<_>>>()?;
    Ok(Some(RecordPermissionPolicy {
        mode,
        grants,
        revision: row.try_get("revision")?,
    }))
}

#[async_trait]
impl RecordPermissionRepository for PgControlPlaneStore {
    async fn get_record_permissions(
        &self,
        key: &RecordPermissionKey,
    ) -> Result<Option<RecordPermissionPolicy>> {
        let mut tx = self.pool().begin().await?;
        // One transaction snapshot for header and grants: a concurrent replace cannot mix them.
        sqlx::query("set transaction isolation level repeatable read, read only")
            .execute(&mut *tx)
            .await?;
        let policy = read_policy(&mut tx, key).await?;
        tx.commit().await?;
        Ok(policy)
    }

    async fn get_record_permissions_batch(
        &self,
        workspace_id: Uuid,
        resource: &RecordPermissionResourceKey,
        record_ids: &[String],
    ) -> Result<BTreeMap<String, RecordPermissionPolicy>> {
        let rows = sqlx::query(
            "select p.record_id, p.mode, p.revision, g.role_id, g.actions
            from plugin_record_permission_policies p
            left join plugin_record_permission_grants g on g.policy_id = p.id
            where p.workspace_id=$1 and p.publisher_namespace=$2 and p.plugin_code=$3
                and p.record_id=any($4) and p.resource_code=$5
            order by p.record_id, g.role_id",
        )
        .bind(workspace_id)
        .bind(&resource.publisher_namespace)
        .bind(&resource.plugin_code)
        .bind(record_ids)
        .bind(&resource.resource_code)
        .fetch_all(self.pool())
        .await?;
        let mut policies = BTreeMap::<String, RecordPermissionPolicy>::new();
        for row in rows {
            let id: String = row.try_get("record_id")?;
            let policy = match policies.entry(id) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(RecordPermissionPolicy {
                        mode: policy_mode(row.try_get("mode")?)?,
                        revision: row.try_get("revision")?,
                        grants: Vec::new(),
                    })
                }
            };
            if let Some(role_id) = row.try_get::<Option<Uuid>, _>("role_id")? {
                policy.grants.push(RecordPermissionGrant {
                    role_id,
                    actions: row.try_get("actions")?,
                });
            }
        }
        Ok(policies)
    }

    async fn replace_record_permissions(
        &self,
        key: &RecordPermissionKey,
        input: &ReplaceRecordPermissions,
        actor_user_id: Uuid,
    ) -> Result<RecordPermissionPolicy> {
        if key.record_id.is_empty()
            || key.record_id.len() > 1024
            || input.grants.len() > 256
            || input.expected_revision.is_some_and(|revision| revision < 0)
            || (input.mode == RecordPermissionMode::Inherit && !input.grants.is_empty())
        {
            return Err(Error::InvalidInput("record_permission_policy").into());
        }
        let mut tx = self.pool().begin().await?;
        // Serialize even the first insert and inherit tombstones; revision is monotonic.
        let lock_key = serde_json::to_string(&(key.workspace_id, &key.resource, &key.record_id))?;
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(lock_key)
            .execute(&mut *tx)
            .await?;
        let old = read_policy(&mut tx, key).await?.unwrap_or_default();
        if input
            .expected_revision
            .is_some_and(|revision| revision != old.revision)
        {
            return Err(Error::Conflict("record_permission_revision").into());
        }
        let mut seen_roles = BTreeSet::new();
        for grant in &input.grants {
            if !seen_roles.insert(grant.role_id)
                || grant.actions.len() > 32
                || grant.actions.iter().any(|action| {
                    action == "create"
                        || action.is_empty()
                        || action.len() > 63
                        || !action.bytes().all(|byte| {
                            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                        })
                })
                || grant.actions.iter().collect::<BTreeSet<_>>().len() != grant.actions.len()
            {
                return Err(Error::InvalidInput("record_permission_grant").into());
            }
            let role = sqlx::query_scalar::<_, Uuid>("select id from roles where id=$1 and scope_kind='workspace' and workspace_id=$2 for key share")
                .bind(grant.role_id).bind(key.workspace_id).fetch_optional(&mut *tx).await?;
            if role.is_none() {
                return Err(Error::InvalidInput("record_permission_role").into());
            }
        }
        let revision = old
            .revision
            .checked_add(1)
            .ok_or(Error::Conflict("record_permission_revision"))?;
        let policy_id: Uuid = sqlx::query_scalar("insert into plugin_record_permission_policies (id,workspace_id,publisher_namespace,plugin_code,resource_code,record_id,mode,revision,created_by,updated_by) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$9) on conflict (workspace_id,publisher_namespace,plugin_code,resource_code,record_id) do update set mode=excluded.mode,revision=excluded.revision,updated_by=excluded.updated_by,updated_at=now() returning id")
            .bind(Uuid::now_v7()).bind(key.workspace_id).bind(&key.resource.publisher_namespace).bind(&key.resource.plugin_code)
            .bind(&key.resource.resource_code).bind(&key.record_id).bind(match input.mode { RecordPermissionMode::Inherit => "inherit", RecordPermissionMode::Restricted => "restricted" })
            .bind(revision).bind(actor_user_id).fetch_one(&mut *tx).await?;
        sqlx::query("delete from plugin_record_permission_grants where policy_id=$1")
            .bind(policy_id)
            .execute(&mut *tx)
            .await?;
        for grant in &input.grants {
            sqlx::query("insert into plugin_record_permission_grants (policy_id,role_id,actions) values ($1,$2,$3)")
                .bind(policy_id).bind(grant.role_id).bind(&grant.actions).execute(&mut *tx).await?;
        }
        let payload = serde_json::json!({"resource":key.resource, "record_id":key.record_id,
            "mode":input.mode, "grants":input.grants, "revision":revision});
        sqlx::query("insert into audit_logs (id,workspace_id,scope_id,actor_user_id,target_type,target_id,event_code,payload,created_by,updated_by) values ($1,$2,$2,$3,'plugin_record_permission',$4,'plugin_record_permission.replaced',$5,$3,$3)")
            .bind(Uuid::now_v7()).bind(key.workspace_id).bind(actor_user_id).bind(policy_id).bind(payload).execute(&mut *tx).await?;
        let policy = read_policy(&mut tx, key)
            .await?
            .expect("policy was inserted in this transaction");
        tx.commit().await?;
        Ok(policy)
    }

    async fn record_permission_effective_role_ids(
        &self,
        actor: &ActorContext,
    ) -> Result<Vec<Uuid>> {
        let ids = self
            .effective_role_ids(
                actor.user_id,
                actor.current_workspace_id,
                &actor.effective_display_role,
            )
            .await?;
        Ok(sqlx::query_scalar("select id from roles where id=any($1) and scope_kind='workspace' and workspace_id=$2 order by id")
            .bind(ids).bind(actor.current_workspace_id).fetch_all(self.pool()).await?)
    }

    async fn list_record_permission_roles(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<RecordPermissionRole>> {
        sqlx::query("select id,code,name from roles where scope_kind='workspace' and workspace_id=$1 order by code,id")
            .bind(workspace_id).fetch_all(self.pool()).await?.into_iter().map(|row| Ok(RecordPermissionRole {
                role_id: row.try_get("id")?, code: row.try_get("code")?, name: row.try_get("name")?,
            })).collect()
    }
}
