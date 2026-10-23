mod change_log;
mod create;
mod field_queries;
mod file_template;
mod model_queries;
mod naming;

pub use naming::RuntimeTableNamePolicy;

use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::{
    ports::{
        AddModelFieldInput, AuthRepository, CreateModelDefinitionInput,
        CreateScopeDataModelGrantInput, ModelDefinitionRepository,
        ReconcileSystemModelDefinitionInput, ReconcileSystemModelFieldInput,
        UpdateModelDefinitionInput, UpdateModelDefinitionStatusInput, UpdateModelFieldInput,
        UpdateScopeDataModelGrantInput,
    },
    ControlPlaneContractError as ControlPlaneError,
};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    mappers::model_definition_mapper::PgModelDefinitionMapper,
    physical_schema_repository::{
        add_fk_column_and_constraint, add_scalar_column, create_join_table, drop_join_table,
        drop_runtime_column, drop_runtime_model_table, is_platform_runtime_column, join_table_name,
    },
    repositories::{tenant_id_for_workspace, workspace_id_for_user, PgControlPlaneStore},
};

use self::{
    change_log::{append_change_log, append_change_log_tx, ChangeLogEntry},
    field_queries::{
        insert_model_field, insert_model_field_after_failure, load_fields_by_model_id,
        load_fields_for_model, load_join_tables_for_model, load_model_field_for_update,
    },
    model_queries::{
        load_model_definition, load_model_definition_for_update, load_model_definition_with_lock,
    },
    naming::{build_physical_column_name, nullable_actor_user_id},
};

fn platform_runtime_field_records(
    model: &domain::ModelDefinitionRecord,
) -> Vec<domain::ModelFieldRecord> {
    let mut fields = [
        ("id", domain::ModelFieldKind::String),
        ("scope_id", domain::ModelFieldKind::ManyToOne),
        ("created_by", domain::ModelFieldKind::String),
        ("updated_by", domain::ModelFieldKind::String),
        ("created_at", domain::ModelFieldKind::Datetime),
        ("updated_at", domain::ModelFieldKind::Datetime),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(sort_order, (code, field_kind))| domain::ModelFieldRecord {
            id: Uuid::now_v7(),
            data_model_id: model.id,
            code: code.to_string(),
            title: code.to_string(),
            description: None,
            physical_column_name: code.to_string(),
            external_field_key: None,
            field_kind,
            is_system: true,
            is_writable: false,
            is_required: true,
            api_required: false,
            is_unique: code == "id",
            default_value: None,
            display_interface: None,
            display_options: serde_json::json!({}),
            relation_target_model_id: None,
            relation_options: serde_json::json!({}),
            sort_order: sort_order as i32,
            availability_status: domain::MetadataAvailabilityStatus::Available,
        },
    )
    .collect::<Vec<_>>();
    fields.extend(crate::ordered_tree::schema::system_field_records(model));
    fields.extend(file_template::field_records(model));
    fields
}

#[async_trait]
impl ModelDefinitionRepository for PgControlPlaneStore {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        let workspace_id = workspace_id_for_user(self.pool(), actor_user_id).await?;
        let tenant_id = tenant_id_for_workspace(self.pool(), workspace_id).await?;
        AuthRepository::load_actor_context(self, actor_user_id, tenant_id, workspace_id, None).await
    }

    async fn list_model_definitions(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::ModelDefinitionRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                scope_kind,
                scope_id,
                data_source_instance_id,
                source_kind,
                external_resource_key,
                external_table_id,
                external_capability_snapshot,
                template_provider,
                template_code,
                template_version,
                code,
                title,
                description,
                physical_table_name,
                acl_namespace,
                audit_namespace,
                availability_status,
                status,
                owner_kind,
                owner_id,
                is_protected
            from model_definitions
            where $1 = '00000000-0000-0000-0000-000000000000'::uuid
               or scope_kind <> 'workspace'
               or scope_id = $1
            order by created_at asc
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        let model_ids = rows.iter().map(|row| row.get("id")).collect::<Vec<Uuid>>();
        let mut fields_by_model_id = load_fields_by_model_id(self.pool(), &model_ids).await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let model_id = row.get("id");
                PgModelDefinitionMapper::from_pg_row(
                    row,
                    fields_by_model_id.remove(&model_id).unwrap_or_default(),
                )
            })
            .collect())
    }

    async fn get_model_definition(
        &self,
        workspace_id: Uuid,
        model_id: Uuid,
    ) -> Result<Option<domain::ModelDefinitionRecord>> {
        let model = load_model_definition(self.pool(), model_id).await?;
        Ok(model.filter(|definition| {
            workspace_id.is_nil()
                || !matches!(definition.scope_kind, domain::DataModelScopeKind::Workspace)
                || definition.scope_id == workspace_id
        }))
    }

    async fn get_data_source_defaults(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
    ) -> Result<domain::DataSourceDefaults> {
        let row = sqlx::query(
            r#"
            select default_data_model_status
            from data_source_instances
            where id = $1
              and workspace_id = $2
            "#,
        )
        .bind(data_source_instance_id)
        .bind(workspace_id)
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::NotFound("data_source_instance"))?;

        Ok(domain::DataSourceDefaults {
            data_model_status: domain::DataModelStatus::from_db(
                row.get::<String, _>("default_data_model_status").as_str(),
            ),
        })
    }

    async fn get_main_source_defaults(
        &self,
        workspace_id: Uuid,
    ) -> Result<domain::DataSourceDefaults> {
        let row = sqlx::query(
            r#"
            select default_data_model_status
            from main_source_defaults
            where workspace_id = $1
            "#,
        )
        .bind(workspace_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row
            .map(|row| domain::DataSourceDefaults {
                data_model_status: domain::DataModelStatus::from_db(
                    row.get::<String, _>("default_data_model_status").as_str(),
                ),
            })
            .unwrap_or_default())
    }

    async fn create_model_definition(
        &self,
        input: &CreateModelDefinitionInput,
    ) -> Result<domain::ModelDefinitionRecord> {
        create::create_model_definition(self, input).await
    }

    async fn update_model_definition(
        &self,
        input: &UpdateModelDefinitionInput,
    ) -> Result<domain::ModelDefinitionRecord> {
        let row = sqlx::query(
            r#"
            update model_definitions
            set title = $2,
                description = $3,
                external_table_id = $4,
                updated_by = $5,
                updated_at = now()
            where id = $1
            returning
                id,
                scope_kind,
                scope_id,
                data_source_instance_id,
                source_kind,
                external_resource_key,
                external_table_id,
                external_capability_snapshot,
                template_provider,
                template_code,
                template_version,
                code,
                title,
                description,
                physical_table_name,
                acl_namespace,
                audit_namespace,
                availability_status,
                status,
                owner_kind,
                owner_id,
                is_protected
            "#,
        )
        .bind(input.model_id)
        .bind(&input.title)
        .bind(&input.description)
        .bind(&input.external_table_id)
        .bind(nullable_actor_user_id(input.actor_user_id))
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let fields = load_fields_for_model(self.pool(), input.model_id).await?;

        Ok(PgModelDefinitionMapper::from_pg_row(row, fields))
    }

    async fn reconcile_system_model_definition(
        &self,
        input: &ReconcileSystemModelDefinitionInput,
    ) -> Result<domain::ModelDefinitionRecord> {
        let row = sqlx::query(
            r#"
            update model_definitions
            set title = $2,
                physical_table_name = $3,
                status = $4,
                owner_kind = $5,
                owner_id = $6,
                is_protected = $7,
                availability_status = 'available',
                updated_by = $8,
                updated_at = now()
            where id = $1
            returning
                id,
                scope_kind,
                scope_id,
                data_source_instance_id,
                source_kind,
                external_resource_key,
                external_table_id,
                external_capability_snapshot,
                template_provider,
                template_code,
                template_version,
                code,
                title,
                description,
                physical_table_name,
                acl_namespace,
                audit_namespace,
                availability_status,
                status,
                owner_kind,
                owner_id,
                is_protected
            "#,
        )
        .bind(input.model_id)
        .bind(&input.title)
        .bind(&input.physical_table_name)
        .bind(input.status.as_str())
        .bind(input.protection.owner_kind.as_str())
        .bind(&input.protection.owner_id)
        .bind(input.protection.is_protected)
        .bind(nullable_actor_user_id(input.actor_user_id))
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let fields = load_fields_for_model(self.pool(), input.model_id).await?;

        Ok(PgModelDefinitionMapper::from_pg_row(row, fields))
    }

    async fn update_model_definition_status(
        &self,
        input: &UpdateModelDefinitionStatusInput,
    ) -> Result<domain::ModelDefinitionRecord> {
        let row = sqlx::query(
            r#"
            update model_definitions
            set status = $2,
                updated_by = $3,
                updated_at = now()
            where id = $1
              and (
                  $4 = '00000000-0000-0000-0000-000000000000'::uuid
                  or scope_kind <> 'workspace'
                  or scope_id = $4
              )
            returning
                id,
                scope_kind,
                scope_id,
                data_source_instance_id,
                source_kind,
                external_resource_key,
                external_table_id,
                external_capability_snapshot,
                template_provider,
                template_code,
                template_version,
                code,
                title,
                description,
                physical_table_name,
                acl_namespace,
                audit_namespace,
                availability_status,
                status,
                owner_kind,
                owner_id,
                is_protected
            "#,
        )
        .bind(input.model_id)
        .bind(input.status.as_str())
        .bind(nullable_actor_user_id(input.actor_user_id))
        .bind(input.workspace_id)
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let fields = load_fields_for_model(self.pool(), input.model_id).await?;

        Ok(PgModelDefinitionMapper::from_pg_row(row, fields))
    }

    async fn add_model_field(
        &self,
        input: &AddModelFieldInput,
    ) -> Result<domain::ModelFieldRecord> {
        self.add_model_field_impl(input).await
    }

    async fn update_model_field(
        &self,
        input: &UpdateModelFieldInput,
    ) -> Result<domain::ModelFieldRecord> {
        self.update_model_field_impl(input).await
    }

    async fn reconcile_system_model_field(
        &self,
        input: &ReconcileSystemModelFieldInput,
    ) -> Result<domain::ModelFieldRecord> {
        self.reconcile_system_model_field_impl(input).await
    }

    async fn delete_model_definition(&self, actor_user_id: Uuid, model_id: Uuid) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        let model = load_model_definition_for_update(&mut tx, model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let related_join_tables = load_join_tables_for_model(&mut tx, model_id).await?;
        let before_snapshot = serde_json::to_value(&model)?;
        let actor_user_id = nullable_actor_user_id(actor_user_id);

        let transactional_result = async {
            for join_table in related_join_tables {
                drop_join_table(&mut tx, &join_table).await?;
            }
            drop_runtime_model_table(&mut tx, &model.physical_table_name).await?;
            sqlx::query("delete from model_definitions where id = $1")
                .bind(model_id)
                .execute(&mut *tx)
                .await?;
            append_change_log_tx(
                &mut tx,
                &ChangeLogEntry {
                    data_model_id: None,
                    action: "model.deleted",
                    target_type: "model_definition",
                    target_id: Some(model_id),
                    actor_user_id,
                    before_snapshot: before_snapshot.clone(),
                    after_snapshot: serde_json::json!({}),
                    execution_status: "success",
                    error_message: None,
                },
            )
            .await
        }
        .await;

        match transactional_result {
            Ok(()) => {
                tx.commit().await?;
                Ok(())
            }
            Err(error) => {
                tx.rollback().await?;
                append_change_log(
                    self.pool(),
                    &ChangeLogEntry {
                        data_model_id: None,
                        action: "model.deleted",
                        target_type: "model_definition",
                        target_id: Some(model_id),
                        actor_user_id,
                        before_snapshot,
                        after_snapshot: serde_json::json!({}),
                        execution_status: "failed",
                        error_message: Some(error.to_string()),
                    },
                )
                .await?;
                Err(error)
            }
        }
    }

    async fn delete_model_field(
        &self,
        actor_user_id: Uuid,
        model_id: Uuid,
        field_id: Uuid,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        let model = load_model_definition_for_update(&mut tx, model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let field = load_model_field_for_update(&mut tx, model_id, field_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_field"))?;
        let relation_target = match field.relation_target_model_id {
            Some(relation_target_model_id) => {
                load_model_definition_with_lock(&mut tx, relation_target_model_id, false).await?
            }
            None => None,
        };
        let before_snapshot = serde_json::to_value(&field)?;
        let actor_user_id = nullable_actor_user_id(actor_user_id);

        let transactional_result = async {
            if model.source_kind == domain::DataModelSourceKind::MainSource {
                match field.field_kind {
                    domain::ModelFieldKind::ManyToMany => {
                        if let Some(relation_target) = relation_target.as_ref() {
                            drop_join_table(
                                &mut tx,
                                &join_table_name(
                                    &model.code,
                                    model.id,
                                    &relation_target.code,
                                    relation_target.id,
                                ),
                            )
                            .await?;
                        }
                    }
                    domain::ModelFieldKind::OneToMany | domain::ModelFieldKind::FilePreviewUrl => {}
                    _ => {
                        drop_runtime_column(
                            &mut tx,
                            &model.physical_table_name,
                            &field.physical_column_name,
                        )
                        .await?;
                    }
                }
            }
            sqlx::query("delete from model_fields where id = $1 and data_model_id = $2")
                .bind(field_id)
                .bind(model_id)
                .execute(&mut *tx)
                .await?;
            append_change_log_tx(
                &mut tx,
                &ChangeLogEntry {
                    data_model_id: Some(model_id),
                    action: "field.deleted",
                    target_type: "model_field",
                    target_id: Some(field_id),
                    actor_user_id,
                    before_snapshot: before_snapshot.clone(),
                    after_snapshot: serde_json::json!({}),
                    execution_status: "success",
                    error_message: None,
                },
            )
            .await
        }
        .await;

        match transactional_result {
            Ok(()) => {
                tx.commit().await?;
                Ok(())
            }
            Err(error) => {
                tx.rollback().await?;
                append_change_log(
                    self.pool(),
                    &ChangeLogEntry {
                        data_model_id: Some(model_id),
                        action: "field.deleted",
                        target_type: "model_field",
                        target_id: Some(field_id),
                        actor_user_id,
                        before_snapshot,
                        after_snapshot: serde_json::json!({}),
                        execution_status: "failed",
                        error_message: Some(error.to_string()),
                    },
                )
                .await?;
                Err(error)
            }
        }
    }

    async fn publish_model_definition(
        &self,
        _actor_user_id: Uuid,
        model_id: Uuid,
    ) -> Result<domain::ModelDefinitionRecord> {
        load_model_definition(self.pool(), model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition").into())
    }

    async fn create_scope_data_model_grant(
        &self,
        input: &CreateScopeDataModelGrantInput,
    ) -> Result<domain::ScopeDataModelGrantRecord> {
        ensure_model_accepts_scope_grant(
            self.pool(),
            input.data_model_id,
            input.scope_kind,
            input.scope_id,
        )
        .await?;

        let row = sqlx::query(
            r#"
            insert into scope_data_model_grants (
                id,
                scope_kind,
                scope_id,
                data_model_id,
                enabled,
                permission_profile,
                created_by
            )
            values ($1, $2, $3, $4, $5, $6, $7)
            returning
                id,
                scope_kind,
                scope_id,
                data_model_id,
                enabled,
                permission_profile,
                created_by,
                created_at,
                updated_at
            "#,
        )
        .bind(input.grant_id)
        .bind(input.scope_kind.as_str())
        .bind(input.scope_id)
        .bind(input.data_model_id)
        .bind(input.enabled)
        .bind(input.permission_profile.as_str())
        .bind(input.created_by)
        .fetch_one(self.pool())
        .await?;

        map_scope_data_model_grant(row)
    }

    async fn update_scope_data_model_grant(
        &self,
        input: &UpdateScopeDataModelGrantInput,
    ) -> Result<domain::ScopeDataModelGrantRecord> {
        let existing_scope =
            load_scope_data_model_grant_scope(self.pool(), input.data_model_id, input.grant_id)
                .await?;
        ensure_model_accepts_scope_grant(
            self.pool(),
            input.data_model_id,
            existing_scope.0,
            existing_scope.1,
        )
        .await?;

        let row = sqlx::query(
            r#"
            update scope_data_model_grants
            set enabled = $3,
                permission_profile = $4,
                updated_at = now()
            where data_model_id = $1
              and id = $2
            returning
                id,
                scope_kind,
                scope_id,
                data_model_id,
                enabled,
                permission_profile,
                created_by,
                created_at,
                updated_at
            "#,
        )
        .bind(input.data_model_id)
        .bind(input.grant_id)
        .bind(input.enabled)
        .bind(input.permission_profile.as_str())
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::NotFound("scope_data_model_grant"))?;

        map_scope_data_model_grant(row)
    }

    async fn get_scope_data_model_grant(
        &self,
        data_model_id: Uuid,
        grant_id: Uuid,
    ) -> Result<Option<domain::ScopeDataModelGrantRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                scope_kind,
                scope_id,
                data_model_id,
                enabled,
                permission_profile,
                created_by,
                created_at,
                updated_at
            from scope_data_model_grants
            where data_model_id = $1
              and id = $2
            "#,
        )
        .bind(data_model_id)
        .bind(grant_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_scope_data_model_grant).transpose()
    }

    async fn delete_scope_data_model_grant(
        &self,
        data_model_id: Uuid,
        grant_id: Uuid,
    ) -> Result<domain::ScopeDataModelGrantRecord> {
        let row = sqlx::query(
            r#"
            delete from scope_data_model_grants
            where data_model_id = $1
              and id = $2
            returning
                id,
                scope_kind,
                scope_id,
                data_model_id,
                enabled,
                permission_profile,
                created_by,
                created_at,
                updated_at
            "#,
        )
        .bind(data_model_id)
        .bind(grant_id)
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::NotFound("scope_data_model_grant"))?;

        map_scope_data_model_grant(row)
    }

    async fn list_scope_data_model_grants(
        &self,
        scope_kind: domain::DataModelScopeKind,
        scope_id: Uuid,
    ) -> Result<Vec<domain::ScopeDataModelGrantRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                scope_kind,
                scope_id,
                data_model_id,
                enabled,
                permission_profile,
                created_by,
                created_at,
                updated_at
            from scope_data_model_grants
            where scope_kind = $1
              and scope_id = $2
            order by created_at asc
            "#,
        )
        .bind(scope_kind.as_str())
        .bind(scope_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_scope_data_model_grant).collect()
    }

    async fn list_actor_role_data_policies(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        role_code: &str,
        data_model_id: Uuid,
    ) -> Result<
        Vec<(
            domain::RoleDataPolicyRecord,
            Option<domain::RoleDataModelPolicyRecord>,
        )>,
    > {
        let role_ids = self
            .effective_role_ids(actor_user_id, workspace_id, role_code)
            .await?;
        let rows = sqlx::query(
            r#"
            select
              r.code as role_code,
              rdp.id as policy_id,
              rdp.role_id,
              rdp.can_view,
              rdp.can_create,
              rdp.can_update,
              rdp.can_delete,
              rdp.default_view_scope,
              rdp.default_update_scope,
              rdp.default_delete_scope,
              rdp.created_at as policy_created_at,
              rdp.updated_at as policy_updated_at,
              rdmp.id as model_policy_id,
              rdmp.data_model_id,
              rdmp.can_create_override,
              rdmp.view_scope_override,
              rdmp.update_scope_override,
              rdmp.delete_scope_override,
              rdmp.created_at as model_policy_created_at,
              rdmp.updated_at as model_policy_updated_at
            from roles r
            join role_data_policies rdp on rdp.role_id = r.id
            left join role_data_model_policies rdmp
              on rdmp.role_id = r.id
             and rdmp.data_model_id = $2
            where r.id = any($1)
            order by r.scope_kind asc, r.code asc
            "#,
        )
        .bind(&role_ids)
        .bind(data_model_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let role_id = row.get("role_id");
                let default_policy = domain::RoleDataPolicyRecord {
                    id: row.get("policy_id"),
                    role_id,
                    role_code: row.get("role_code"),
                    can_view: row.get("can_view"),
                    can_create: row.get("can_create"),
                    can_update: row.get("can_update"),
                    can_delete: row.get("can_delete"),
                    default_view_scope: domain::RoleDataPolicyScope::from_db(
                        row.get::<String, _>("default_view_scope").as_str(),
                    ),
                    default_update_scope: domain::RoleDataPolicyScope::from_db(
                        row.get::<String, _>("default_update_scope").as_str(),
                    ),
                    default_delete_scope: domain::RoleDataPolicyScope::from_db(
                        row.get::<String, _>("default_delete_scope").as_str(),
                    ),
                    created_at: row.get("policy_created_at"),
                    updated_at: row.get("policy_updated_at"),
                };
                let model_policy_id: Option<Uuid> = row.get("model_policy_id");
                let model_policy = model_policy_id.map(|id| domain::RoleDataModelPolicyRecord {
                    id,
                    role_id,
                    data_model_id: row.get("data_model_id"),
                    can_create_override: row.get("can_create_override"),
                    view_scope_override: row
                        .get::<Option<String>, _>("view_scope_override")
                        .map(|scope| domain::RoleDataPolicyScope::from_db(&scope)),
                    update_scope_override: row
                        .get::<Option<String>, _>("update_scope_override")
                        .map(|scope| domain::RoleDataPolicyScope::from_db(&scope)),
                    delete_scope_override: row
                        .get::<Option<String>, _>("delete_scope_override")
                        .map(|scope| domain::RoleDataPolicyScope::from_db(&scope)),
                    created_at: row.get("model_policy_created_at"),
                    updated_at: row.get("model_policy_updated_at"),
                });
                (default_policy, model_policy)
            })
            .collect())
    }

    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> Result<()> {
        AuthRepository::append_audit_log(self, event).await
    }
}

async fn ensure_model_accepts_scope_grant(
    pool: &sqlx::PgPool,
    model_id: Uuid,
    grant_scope_kind: domain::DataModelScopeKind,
    grant_scope_id: Uuid,
) -> Result<()> {
    let model_scope = sqlx::query_as::<_, (String, Uuid)>(
        r#"
        select scope_kind, scope_id
        from model_definitions
        where id = $1
        "#,
    )
    .bind(model_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ControlPlaneError::NotFound("model_definition"))?;
    let model_scope_kind = domain::DataModelScopeKind::from_db(&model_scope.0);

    if model_scope_kind == domain::DataModelScopeKind::Workspace
        && (grant_scope_kind != domain::DataModelScopeKind::Workspace
            || grant_scope_id != model_scope.1)
    {
        return Err(
            ControlPlaneError::PermissionDenied("workspace_model_scope_grant_mismatch").into(),
        );
    }

    Ok(())
}

async fn load_scope_data_model_grant_scope(
    pool: &sqlx::PgPool,
    model_id: Uuid,
    grant_id: Uuid,
) -> Result<(domain::DataModelScopeKind, Uuid)> {
    let (scope_kind, scope_id) = sqlx::query_as::<_, (String, Uuid)>(
        r#"
        select scope_kind, scope_id
        from scope_data_model_grants
        where data_model_id = $1
          and id = $2
        "#,
    )
    .bind(model_id)
    .bind(grant_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ControlPlaneError::NotFound("scope_data_model_grant"))?;

    Ok((domain::DataModelScopeKind::from_db(&scope_kind), scope_id))
}

fn map_scope_data_model_grant(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ScopeDataModelGrantRecord> {
    Ok(domain::ScopeDataModelGrantRecord {
        id: row.get("id"),
        scope_kind: domain::DataModelScopeKind::from_db(
            row.get::<String, _>("scope_kind").as_str(),
        ),
        scope_id: row.get("scope_id"),
        data_model_id: row.get("data_model_id"),
        enabled: row.get("enabled"),
        permission_profile: domain::ScopeDataModelPermissionProfile::from_db(
            row.get::<String, _>("permission_profile").as_str(),
        ),
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

mod field_mutations;
