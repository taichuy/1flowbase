use crate::{
    ordered_tree::commands::{
        create_ordered_tree_node_in_transaction, delete_ordered_tree_leaf_in_transaction,
        move_ordered_tree_node_in_transaction,
    },
    repositories::PgControlPlaneStore,
};
use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::{
    ports::{OrganizationRepository, SaveDepartmentInput},
    ControlPlaneContractError as Error,
};
use domain::{Department, MemberDepartments};
use sqlx::{Postgres, Row, Transaction};
use storage_durable::{
    model_metadata::ModelMetadata,
    resource_descriptor::ResourceDescriptor,
    runtime_record_repository::{
        OrderedTreeCommandError, OrderedTreeCreateInput, OrderedTreeCreatePosition,
        OrderedTreeLeafDeleteInput, OrderedTreeMoveInput, OrderedTreeMovePosition,
    },
};
use uuid::Uuid;

fn metadata(scope_id: Uuid) -> ModelMetadata {
    let field = domain::ModelFieldRecord {
        id: Uuid::nil(),
        data_model_id: domain::DEPARTMENT_MODEL_ID,
        code: "name".into(),
        title: "Name".into(),
        description: None,
        physical_column_name: "name".into(),
        external_field_key: None,
        field_kind: domain::ModelFieldKind::String,
        is_system: false,
        is_writable: true,
        is_required: true,
        api_required: true,
        is_unique: false,
        default_value: None,
        display_interface: None,
        display_options: serde_json::json!({}),
        relation_target_model_id: None,
        relation_options: serde_json::json!({}),
        sort_order: 10,
        availability_status: domain::MetadataAvailabilityStatus::Available,
    };
    ModelMetadata {
        model_id: domain::DEPARTMENT_MODEL_ID,
        model_code: "departments".into(),
        status: domain::DataModelStatus::Published,
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id,
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        external_capability_snapshot: None,
        template_provider: "core".into(),
        template_code: "ordered_tree".into(),
        template_version: "v1".into(),
        physical_table_name: "departments".into(),
        scope_column_name: "scope_id".into(),
        fields: vec![field],
        record_capabilities: domain::DataModelRecordCapabilities::read_write(),
        resource: ResourceDescriptor::runtime_model(
            "departments",
            domain::DataModelScopeKind::Workspace,
        ),
    }
}

// All organization writers acquire the same workspace lock before tree/row locks.
async fn lock(tx: &mut Transaction<'_, Postgres>, scope_id: Uuid) -> Result<()> {
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('organization:' || $1::text, 0))")
        .bind(scope_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn tree_error(error: anyhow::Error) -> anyhow::Error {
    if let Some(tree) = error.downcast_ref::<OrderedTreeCommandError>() {
        match tree {
            OrderedTreeCommandError::NodeNotFound => return Error::NotFound("department").into(),
            OrderedTreeCommandError::ParentNotFound => {
                return Error::InvalidInput("parent_id").into()
            }
            OrderedTreeCommandError::Cycle => return Error::Conflict("department_cycle").into(),
            OrderedTreeCommandError::TreeNodeHasChildren => {
                return Error::Conflict("department_has_children").into()
            }
            _ => {}
        }
    }
    error
}

#[async_trait]
impl OrganizationRepository for PgControlPlaneStore {
    async fn list_departments(&self, workspace_id: Uuid) -> Result<Vec<Department>> {
        let rows = sqlx::query(r#"
            with recursive descendants(root_id,id) as (
                select id,id from departments where scope_id=$1
                union all
                select d.root_id,c.id from descendants d join departments c on c.parent_id=d.id where c.scope_id=$1
            )
            select d.id,d.name,d.parent_id,
                array(select r.code from department_role_bindings b join roles r on r.id=b.role_id
                      where b.scope_id=$1 and b.department_id=d.id order by r.code) role_codes,
                (select count(distinct b.user_id) from descendants s join user_department_bindings b
                    on b.department_id=s.id and b.scope_id=$1 where s.root_id=d.id) member_count
            from departments d where scope_id=$1 order by sibling_rank collate "C",id
        "#).bind(workspace_id).fetch_all(self.pool()).await?;
        rows.into_iter()
            .map(|r| {
                Ok(Department {
                    id: r.try_get("id")?,
                    name: r.try_get("name")?,
                    parent_id: r.try_get("parent_id")?,
                    role_codes: r.try_get("role_codes")?,
                    member_count: r.try_get("member_count")?,
                })
            })
            .collect()
    }

    async fn save_department(&self, input: &SaveDepartmentInput) -> Result<Department> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(Error::InvalidInput("name").into());
        }
        let mut tx = self.pool().begin().await?;
        lock(&mut tx, input.workspace_id).await?;
        let codes: std::collections::BTreeSet<_> = input.role_codes.iter().collect();
        if !input.can_assign_roles {
            let existing: Vec<String> = if let Some(id) = input.department_id {
                sqlx::query_scalar("select r.code from department_role_bindings b join roles r on r.id=b.role_id where b.scope_id=$1 and b.department_id=$2 order by r.code")
                    .bind(input.workspace_id).bind(id).fetch_all(&mut *tx).await?
            } else {
                Vec::new()
            };
            if existing.iter().collect::<std::collections::BTreeSet<_>>() != codes {
                return Err(Error::PermissionDenied("permission_denied").into());
            }
        }

        let mut roles = Vec::new();
        for code in codes {
            let role: Option<Uuid> = sqlx::query_scalar("select id from roles where scope_kind='workspace' and workspace_id=$1 and scope_id=$1 and code=$2 and code <> 'root' and system_kind is null for share")
                .bind(input.workspace_id).bind(code).fetch_optional(&mut *tx).await?;
            roles.push(role.ok_or(Error::InvalidInput("role_codes"))?);
        }
        let model = metadata(input.workspace_id);
        let id = if let Some(id) = input.department_id {
            let parent: Option<Option<Uuid>> = sqlx::query_scalar(
                "select parent_id from departments where scope_id=$1 and id=$2 for update",
            )
            .bind(input.workspace_id)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            let parent = parent.ok_or(Error::NotFound("department"))?;
            if parent != input.parent_id {
                move_ordered_tree_node_in_transaction(
                    &mut tx,
                    &model,
                    OrderedTreeMoveInput {
                        actor_user_id: input.actor_user_id,
                        scope_id: input.workspace_id,
                        tree_partition_id: input.workspace_id,
                        node_id: id,
                        position: OrderedTreeMovePosition {
                            new_parent_id: input.parent_id,
                            before_id: None,
                            after_id: None,
                        },
                    },
                )
                .await
                .map_err(tree_error)?;
            }
            sqlx::query("update departments set name=$3,updated_by=$4,updated_at=now() where scope_id=$1 and id=$2")
                .bind(input.workspace_id).bind(id).bind(name).bind(input.actor_user_id).execute(&mut *tx).await?;
            id
        } else {
            create_ordered_tree_node_in_transaction(
                &mut tx,
                &model,
                OrderedTreeCreateInput {
                    actor_user_id: input.actor_user_id,
                    scope_id: input.workspace_id,
                    tree_partition_id: input.workspace_id,
                    position: OrderedTreeCreatePosition {
                        parent_id: input.parent_id,
                        before_id: None,
                        after_id: None,
                    },
                    payload: serde_json::json!({"name":name}),
                },
            )
            .await
            .map_err(tree_error)?
            .node_id
        };
        sqlx::query("delete from department_role_bindings where scope_id=$1 and department_id=$2")
            .bind(input.workspace_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for role_id in roles {
            sqlx::query("insert into department_role_bindings(id,scope_id,department_id,role_id) values($1,$2,$3,$4)")
                .bind(Uuid::now_v7()).bind(input.workspace_id).bind(id).bind(role_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        self.list_departments(input.workspace_id)
            .await?
            .into_iter()
            .find(|d| d.id == id)
            .ok_or_else(|| Error::NotFound("department").into())
    }

    async fn delete_department(&self, workspace_id: Uuid, department_id: Uuid) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        lock(&mut tx, workspace_id).await?;
        let populated: bool = sqlx::query_scalar("select exists(select 1 from user_department_bindings where scope_id=$1 and department_id=$2)")
            .bind(workspace_id).bind(department_id).fetch_one(&mut *tx).await?;
        if populated {
            return Err(Error::Conflict("department_has_members").into());
        }
        if !delete_ordered_tree_leaf_in_transaction(
            &mut tx,
            &metadata(workspace_id),
            OrderedTreeLeafDeleteInput {
                scope_id: workspace_id,
                tree_partition_id: workspace_id,
                node_id: department_id,
            },
        )
        .await
        .map_err(tree_error)?
        {
            return Err(Error::NotFound("department").into());
        }
        tx.commit().await?;
        Ok(())
    }

    async fn member_departments(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<MemberDepartments> {
        let rows: Vec<(Uuid,bool)> = sqlx::query_as("select department_id,is_primary from user_department_bindings where scope_id=$1 and user_id=$2 order by department_id")
            .bind(workspace_id).bind(user_id).fetch_all(self.pool()).await?;
        Ok(MemberDepartments {
            primary_department_id: rows.iter().find(|(_, primary)| *primary).map(|(id, _)| *id),
            department_ids: rows.into_iter().map(|(id, _)| id).collect(),
        })
    }

    async fn department_member_ids(
        &self,
        workspace_id: Uuid,
        department_id: Uuid,
    ) -> Result<Vec<Uuid>> {
        let exists: bool = sqlx::query_scalar(
            "select exists(select 1 from departments where scope_id=$1 and id=$2)",
        )
        .bind(workspace_id)
        .bind(department_id)
        .fetch_one(self.pool())
        .await?;
        if !exists {
            return Err(Error::NotFound("department").into());
        }
        Ok(sqlx::query_scalar(r#"with recursive subtree(id) as (
            select id from departments where scope_id=$1 and id=$2 union all
            select c.id from departments c join subtree s on c.parent_id=s.id where c.scope_id=$1
        ) select distinct b.user_id from user_department_bindings b join subtree s on s.id=b.department_id where b.scope_id=$1"#)
            .bind(workspace_id).bind(department_id).fetch_all(self.pool()).await?)
    }

    async fn replace_member_departments(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        input: &MemberDepartments,
    ) -> Result<()> {
        if !input.is_valid() {
            return Err(Error::InvalidInput("primary_department_id").into());
        }
        let mut tx = self.pool().begin().await?;
        lock(&mut tx, workspace_id).await?;
        let member: Option<Uuid> = sqlx::query_scalar("select user_id from workspace_memberships where workspace_id=$1 and user_id=$2 for update")
            .bind(workspace_id).bind(user_id).fetch_optional(&mut *tx).await?;
        if member.is_none() {
            return Err(Error::InvalidInput("member_scope").into());
        }
        let count: i64 =
            sqlx::query_scalar("select count(*) from departments where scope_id=$1 and id=any($2)")
                .bind(workspace_id)
                .bind(&input.department_ids)
                .fetch_one(&mut *tx)
                .await?;
        if count != input.department_ids.len() as i64 {
            return Err(Error::InvalidInput("department_ids").into());
        }
        sqlx::query("delete from user_department_bindings where scope_id=$1 and user_id=$2")
            .bind(workspace_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        for id in &input.department_ids {
            sqlx::query("insert into user_department_bindings(id,scope_id,user_id,department_id,is_primary) values($1,$2,$3,$4,$5)")
                .bind(Uuid::now_v7()).bind(workspace_id).bind(user_id).bind(id).bind(input.primary_department_id==Some(*id)).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
