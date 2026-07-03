mod change_log;
mod field_queries;
mod model_queries;
mod naming;

use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        AddModelFieldInput, AuthRepository, CreateModelDefinitionInput,
        CreateScopeDataModelGrantInput, ModelDefinitionRepository,
        ReconcileSystemModelDefinitionInput, ReconcileSystemModelFieldInput,
        UpdateModelDefinitionInput, UpdateModelDefinitionStatusInput, UpdateModelFieldInput,
        UpdateScopeDataModelGrantInput,
    },
};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    mappers::model_definition_mapper::{PgModelDefinitionMapper, StoredModelDefinitionRow},
    physical_schema_repository::{
        add_fk_column_and_constraint, add_scalar_column, create_join_table,
        create_runtime_model_table, drop_join_table, drop_runtime_column, drop_runtime_model_table,
        is_platform_runtime_column, join_table_name,
    },
    repositories::{tenant_id_for_workspace, workspace_id_for_user, PgControlPlaneStore},
};

use self::{
    change_log::{append_change_log, append_change_log_tx, ChangeLogEntry},
    field_queries::{
        insert_model_field, insert_model_field_after_failure, load_fields_by_model_id,
        load_join_tables_for_model, load_model_field_for_update,
    },
    model_queries::{
        insert_model_definition, insert_model_definition_after_failure, load_model_definition,
        load_model_definition_for_update, load_model_definition_with_lock,
    },
    naming::{
        build_physical_column_name, build_physical_table_name, is_registered_system_table,
        nullable_actor_user_id, registered_system_table_name,
    },
};

async fn ensure_workspace_data_source_belongs_to_scope(
    store: &PgControlPlaneStore,
    input: &CreateModelDefinitionInput,
) -> Result<()> {
    if !matches!(input.scope_kind, domain::DataModelScopeKind::Workspace) {
        return Ok(());
    }

    let Some(data_source_instance_id) = input.data_source_instance_id else {
        return Ok(());
    };

    let exists = sqlx::query_scalar::<_, bool>(
        r#"
        select exists (
            select 1
            from data_source_instances
            where id = $1
              and workspace_id = $2
        )
        "#,
    )
    .bind(data_source_instance_id)
    .bind(input.scope_id)
    .fetch_one(store.pool())
    .await?;

    if exists {
        Ok(())
    } else {
        Err(ControlPlaneError::NotFound("data_source_instance").into())
    }
}

fn platform_runtime_field_records(model_id: Uuid) -> Vec<domain::ModelFieldRecord> {
    [
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
            data_model_id: model_id,
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
    .collect()
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
        let fields_by_model_id = load_fields_by_model_id(self.pool()).await?;
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
                code,
                title,
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

        Ok(rows
            .into_iter()
            .map(|row| {
                let model_id: Uuid = row.get("id");
                PgModelDefinitionMapper::to_model_definition_record(StoredModelDefinitionRow {
                    id: model_id,
                    scope_kind: row.get("scope_kind"),
                    scope_id: row.get("scope_id"),
                    data_source_instance_id: row.get("data_source_instance_id"),
                    source_kind: row.get("source_kind"),
                    external_resource_key: row.get("external_resource_key"),
                    external_table_id: row.get("external_table_id"),
                    external_capability_snapshot: row.get("external_capability_snapshot"),
                    code: row.get("code"),
                    title: row.get("title"),
                    physical_table_name: row.get("physical_table_name"),
                    acl_namespace: row.get("acl_namespace"),
                    audit_namespace: row.get("audit_namespace"),
                    availability_status: row.get("availability_status"),
                    status: row.get("status"),
                    owner_kind: row.get("owner_kind"),
                    owner_id: row.get("owner_id"),
                    is_protected: row.get("is_protected"),
                    fields: fields_by_model_id
                        .get(&model_id)
                        .cloned()
                        .unwrap_or_default(),
                })
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
        ensure_workspace_data_source_belongs_to_scope(self, input).await?;

        let mut model = domain::ModelDefinitionRecord {
            id: Uuid::now_v7(),
            scope_kind: input.scope_kind,
            scope_id: input.scope_id,
            data_source_instance_id: input.data_source_instance_id,
            source_kind: input.source_kind,
            external_resource_key: input.external_resource_key.clone(),
            external_table_id: input.external_table_id.clone(),
            external_capability_snapshot: input.external_capability_snapshot.clone(),
            code: input.code.clone(),
            title: input.title.clone(),
            physical_table_name: registered_system_table_name(
                input.scope_kind,
                input.source_kind,
                &input.protection,
                &input.code,
            )
            .map(str::to_string)
            .unwrap_or_else(|| build_physical_table_name(input.scope_kind, &input.code)),
            acl_namespace: format!("state_model.{}", input.code),
            audit_namespace: format!("audit.state_model.{}", input.code),
            fields: vec![],
            availability_status: domain::MetadataAvailabilityStatus::Available,
            status: input.status,
            protection: input.protection.clone(),
        };
        if model.source_kind == domain::DataModelSourceKind::MainSource
            && !is_registered_system_table(&model)
        {
            model.fields = platform_runtime_field_records(model.id);
        }
        let before_snapshot = serde_json::json!({});
        let after_snapshot = serde_json::to_value(&model)?;
        let actor_user_id = nullable_actor_user_id(input.actor_user_id);
        let mut tx = self.pool().begin().await?;

        let transactional_result = async {
            insert_model_definition(
                &mut tx,
                &model,
                actor_user_id,
                domain::MetadataAvailabilityStatus::Available,
            )
            .await?;
            if model.source_kind == domain::DataModelSourceKind::MainSource {
                if !is_registered_system_table(&model) {
                    create_runtime_model_table(&mut tx, &model).await?;
                }
                for field in &model.fields {
                    insert_model_field(
                        &mut tx,
                        field,
                        actor_user_id,
                        domain::MetadataAvailabilityStatus::Available,
                    )
                    .await?;
                }
            }
            append_change_log_tx(
                &mut tx,
                &ChangeLogEntry {
                    data_model_id: Some(model.id),
                    action: "model.created",
                    target_type: "model_definition",
                    target_id: Some(model.id),
                    actor_user_id,
                    before_snapshot: before_snapshot.clone(),
                    after_snapshot: after_snapshot.clone(),
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
                Ok(model)
            }
            Err(error) => {
                tx.rollback().await?;
                insert_model_definition_after_failure(
                    self.pool(),
                    &model,
                    actor_user_id,
                    domain::MetadataAvailabilityStatus::Broken,
                )
                .await?;
                append_change_log(
                    self.pool(),
                    &ChangeLogEntry {
                        data_model_id: None,
                        action: "model.created",
                        target_type: "model_definition",
                        target_id: Some(model.id),
                        actor_user_id,
                        before_snapshot,
                        after_snapshot,
                        execution_status: "failed",
                        error_message: Some(error.to_string()),
                    },
                )
                .await?;
                Err(error)
            }
        }
    }

    async fn update_model_definition(
        &self,
        input: &UpdateModelDefinitionInput,
    ) -> Result<domain::ModelDefinitionRecord> {
        let row = sqlx::query(
            r#"
            update model_definitions
            set title = $2,
                external_table_id = $3,
                updated_by = $4,
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
                code,
                title,
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
        .bind(&input.external_table_id)
        .bind(nullable_actor_user_id(input.actor_user_id))
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let fields_by_model_id = load_fields_by_model_id(self.pool()).await?;

        Ok(PgModelDefinitionMapper::to_model_definition_record(
            StoredModelDefinitionRow {
                id: row.get("id"),
                scope_kind: row.get("scope_kind"),
                scope_id: row.get("scope_id"),
                data_source_instance_id: row.get("data_source_instance_id"),
                source_kind: row.get("source_kind"),
                external_resource_key: row.get("external_resource_key"),
                external_table_id: row.get("external_table_id"),
                external_capability_snapshot: row.get("external_capability_snapshot"),
                code: row.get("code"),
                title: row.get("title"),
                physical_table_name: row.get("physical_table_name"),
                acl_namespace: row.get("acl_namespace"),
                audit_namespace: row.get("audit_namespace"),
                availability_status: row.get("availability_status"),
                status: row.get("status"),
                owner_kind: row.get("owner_kind"),
                owner_id: row.get("owner_id"),
                is_protected: row.get("is_protected"),
                fields: fields_by_model_id
                    .get(&input.model_id)
                    .cloned()
                    .unwrap_or_default(),
            },
        ))
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
                code,
                title,
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
        let fields_by_model_id = load_fields_by_model_id(self.pool()).await?;

        Ok(PgModelDefinitionMapper::to_model_definition_record(
            StoredModelDefinitionRow {
                id: row.get("id"),
                scope_kind: row.get("scope_kind"),
                scope_id: row.get("scope_id"),
                data_source_instance_id: row.get("data_source_instance_id"),
                source_kind: row.get("source_kind"),
                external_resource_key: row.get("external_resource_key"),
                external_table_id: row.get("external_table_id"),
                external_capability_snapshot: row.get("external_capability_snapshot"),
                code: row.get("code"),
                title: row.get("title"),
                physical_table_name: row.get("physical_table_name"),
                acl_namespace: row.get("acl_namespace"),
                audit_namespace: row.get("audit_namespace"),
                availability_status: row.get("availability_status"),
                status: row.get("status"),
                owner_kind: row.get("owner_kind"),
                owner_id: row.get("owner_id"),
                is_protected: row.get("is_protected"),
                fields: fields_by_model_id
                    .get(&input.model_id)
                    .cloned()
                    .unwrap_or_default(),
            },
        ))
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
                code,
                title,
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
        let fields_by_model_id = load_fields_by_model_id(self.pool()).await?;

        Ok(PgModelDefinitionMapper::to_model_definition_record(
            StoredModelDefinitionRow {
                id: row.get("id"),
                scope_kind: row.get("scope_kind"),
                scope_id: row.get("scope_id"),
                data_source_instance_id: row.get("data_source_instance_id"),
                source_kind: row.get("source_kind"),
                external_resource_key: row.get("external_resource_key"),
                external_table_id: row.get("external_table_id"),
                external_capability_snapshot: row.get("external_capability_snapshot"),
                code: row.get("code"),
                title: row.get("title"),
                physical_table_name: row.get("physical_table_name"),
                acl_namespace: row.get("acl_namespace"),
                audit_namespace: row.get("audit_namespace"),
                availability_status: row.get("availability_status"),
                status: row.get("status"),
                owner_kind: row.get("owner_kind"),
                owner_id: row.get("owner_id"),
                is_protected: row.get("is_protected"),
                fields: fields_by_model_id
                    .get(&input.model_id)
                    .cloned()
                    .unwrap_or_default(),
            },
        ))
    }

    async fn add_model_field(
        &self,
        input: &AddModelFieldInput,
    ) -> Result<domain::ModelFieldRecord> {
        let mut tx = self.pool().begin().await?;
        let model = load_model_definition_for_update(&mut tx, input.model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let relation_target = match input.relation_target_model_id {
            Some(relation_target_model_id) => Some(
                load_model_definition_with_lock(&mut tx, relation_target_model_id, false)
                    .await?
                    .ok_or(ControlPlaneError::NotFound("relation_target_model"))?,
            ),
            None => None,
        };
        let physical_column_name = input
            .physical_column_name
            .clone()
            .unwrap_or_else(|| build_physical_column_name(&input.code));
        if input.apply_physical_schema && is_platform_runtime_column(&physical_column_name) {
            return Err(ControlPlaneError::InvalidInput("physical_column_name").into());
        }

        let field = domain::ModelFieldRecord {
            id: Uuid::now_v7(),
            data_model_id: model.id,
            code: input.code.clone(),
            title: input.title.clone(),
            description: input.description.clone(),
            physical_column_name,
            external_field_key: input.external_field_key.clone(),
            field_kind: input.field_kind,
            is_system: input.is_system,
            is_writable: input.is_writable,
            is_required: input.is_required,
            api_required: input.api_required,
            is_unique: input.is_unique,
            default_value: input.default_value.clone(),
            display_interface: input.display_interface.clone(),
            display_options: input.display_options.clone(),
            relation_target_model_id: input.relation_target_model_id,
            relation_options: input.relation_options.clone(),
            sort_order: model.fields.len() as i32,
            availability_status: domain::MetadataAvailabilityStatus::Available,
        };
        let before_snapshot = serde_json::json!({});
        let after_snapshot = serde_json::to_value(&field)?;
        let actor_user_id = nullable_actor_user_id(input.actor_user_id);

        let transactional_result = async {
            insert_model_field(
                &mut tx,
                &field,
                actor_user_id,
                domain::MetadataAvailabilityStatus::Available,
            )
            .await?;
            if model.source_kind == domain::DataModelSourceKind::MainSource
                && input.apply_physical_schema
            {
                match field.field_kind {
                    domain::ModelFieldKind::ManyToOne => {
                        let target = relation_target
                            .as_ref()
                            .ok_or(ControlPlaneError::InvalidInput("relation_target_model_id"))?;
                        add_fk_column_and_constraint(&mut tx, &model, &field, target).await?;
                    }
                    domain::ModelFieldKind::OneToMany => {}
                    domain::ModelFieldKind::ManyToMany => {
                        let target = relation_target
                            .as_ref()
                            .ok_or(ControlPlaneError::InvalidInput("relation_target_model_id"))?;
                        create_join_table(&mut tx, &model, target).await?;
                    }
                    _ => {
                        add_scalar_column(&mut tx, &model, &field).await?;
                    }
                }
            }
            append_change_log_tx(
                &mut tx,
                &ChangeLogEntry {
                    data_model_id: Some(model.id),
                    action: "field.created",
                    target_type: "model_field",
                    target_id: Some(field.id),
                    actor_user_id,
                    before_snapshot: before_snapshot.clone(),
                    after_snapshot: after_snapshot.clone(),
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
                Ok(field)
            }
            Err(error) => {
                tx.rollback().await?;
                insert_model_field_after_failure(
                    self.pool(),
                    &field,
                    actor_user_id,
                    domain::MetadataAvailabilityStatus::Broken,
                )
                .await?;
                append_change_log(
                    self.pool(),
                    &ChangeLogEntry {
                        data_model_id: Some(model.id),
                        action: "field.created",
                        target_type: "model_field",
                        target_id: Some(field.id),
                        actor_user_id,
                        before_snapshot,
                        after_snapshot,
                        execution_status: "failed",
                        error_message: Some(error.to_string()),
                    },
                )
                .await?;
                Err(error)
            }
        }
    }

    async fn update_model_field(
        &self,
        input: &UpdateModelFieldInput,
    ) -> Result<domain::ModelFieldRecord> {
        let mut tx = self.pool().begin().await?;
        let existing = load_model_field_for_update(&mut tx, input.model_id, input.field_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_field"))?;
        let before_snapshot = serde_json::to_value(&existing)?;
        let updated = domain::ModelFieldRecord {
            title: input.title.clone(),
            description: input.description.clone(),
            is_required: input.is_required,
            api_required: input.api_required,
            is_unique: input.is_unique,
            default_value: input.default_value.clone(),
            display_interface: input.display_interface.clone(),
            display_options: input.display_options.clone(),
            relation_options: input.relation_options.clone(),
            ..existing
        };
        let after_snapshot = serde_json::to_value(&updated)?;
        let actor_user_id = nullable_actor_user_id(input.actor_user_id);

        let transactional_result = async {
            sqlx::query(
                r#"
                update model_fields
                set
                    title = $3,
                    description = $4,
                    is_required = $5,
                    api_required = $6,
                    is_unique = $7,
                    default_value = $8,
                    display_interface = $9,
                    display_options = $10,
                    relation_options = $11,
                    updated_by = $12,
                    updated_at = now()
                where id = $1
                  and data_model_id = $2
                "#,
            )
            .bind(input.field_id)
            .bind(input.model_id)
            .bind(&updated.title)
            .bind(&updated.description)
            .bind(updated.is_required)
            .bind(updated.api_required)
            .bind(updated.is_unique)
            .bind(&updated.default_value)
            .bind(&updated.display_interface)
            .bind(&updated.display_options)
            .bind(&updated.relation_options)
            .bind(actor_user_id)
            .execute(&mut *tx)
            .await?;
            append_change_log_tx(
                &mut tx,
                &ChangeLogEntry {
                    data_model_id: Some(input.model_id),
                    action: "field.updated",
                    target_type: "model_field",
                    target_id: Some(input.field_id),
                    actor_user_id,
                    before_snapshot: before_snapshot.clone(),
                    after_snapshot: after_snapshot.clone(),
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
                Ok(updated)
            }
            Err(error) => {
                tx.rollback().await?;
                append_change_log(
                    self.pool(),
                    &ChangeLogEntry {
                        data_model_id: Some(input.model_id),
                        action: "field.updated",
                        target_type: "model_field",
                        target_id: Some(input.field_id),
                        actor_user_id,
                        before_snapshot,
                        after_snapshot,
                        execution_status: "failed",
                        error_message: Some(error.to_string()),
                    },
                )
                .await?;
                Err(error)
            }
        }
    }

    async fn reconcile_system_model_field(
        &self,
        input: &ReconcileSystemModelFieldInput,
    ) -> Result<domain::ModelFieldRecord> {
        let row = sqlx::query(
            r#"
            update model_fields
            set
                title = $3,
                description = $4,
                physical_column_name = $5,
                external_field_key = $6,
                field_kind = $7,
                is_system = $8,
                is_writable = $9,
                is_required = $10,
                api_required = $11,
                is_unique = $12,
                default_value = $13,
                display_interface = $14,
                display_options = $15,
                relation_target_model_id = $16,
                relation_options = $17,
                sort_order = $18,
                availability_status = $19,
                updated_by = $20,
                updated_at = now()
            where id = $1
              and data_model_id = $2
            returning
                id,
                data_model_id,
                code,
                title,
                description,
                physical_column_name,
                external_field_key,
                field_kind,
                is_system,
                is_writable,
                is_required,
                api_required,
                is_unique,
                default_value,
                display_interface,
                display_options,
                relation_target_model_id,
                relation_options,
                sort_order,
                availability_status
            "#,
        )
        .bind(input.field_id)
        .bind(input.model_id)
        .bind(&input.title)
        .bind(&input.description)
        .bind(&input.physical_column_name)
        .bind(&input.external_field_key)
        .bind(input.field_kind.as_str())
        .bind(input.is_system)
        .bind(input.is_writable)
        .bind(input.is_required)
        .bind(input.api_required)
        .bind(input.is_unique)
        .bind(&input.default_value)
        .bind(&input.display_interface)
        .bind(&input.display_options)
        .bind(input.relation_target_model_id)
        .bind(&input.relation_options)
        .bind(input.sort_order)
        .bind(input.availability_status.as_str())
        .bind(nullable_actor_user_id(input.actor_user_id))
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::NotFound("model_field"))?;

        Ok(domain::ModelFieldRecord {
            id: row.get("id"),
            data_model_id: row.get("data_model_id"),
            code: row.get("code"),
            title: row.get("title"),
            description: row.get("description"),
            physical_column_name: row.get("physical_column_name"),
            external_field_key: row.get("external_field_key"),
            field_kind: domain::ModelFieldKind::from_db(
                row.get::<String, _>("field_kind").as_str(),
            ),
            is_system: row.get("is_system"),
            is_writable: row.get("is_writable"),
            is_required: row.get("is_required"),
            api_required: row.get("api_required"),
            is_unique: row.get("is_unique"),
            default_value: row.get("default_value"),
            display_interface: row.get("display_interface"),
            display_options: row.get("display_options"),
            relation_target_model_id: row.get("relation_target_model_id"),
            relation_options: row.get("relation_options"),
            sort_order: row.get("sort_order"),
            availability_status: domain::MetadataAvailabilityStatus::from_db(
                row.get::<String, _>("availability_status").as_str(),
            ),
        })
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
                    domain::ModelFieldKind::OneToMany => {}
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
        ensure_system_model_definition(self.pool(), input.data_model_id).await?;

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
        ensure_system_model_definition(self.pool(), input.data_model_id).await?;

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

    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> Result<()> {
        AuthRepository::append_audit_log(self, event).await
    }
}

async fn ensure_system_model_definition(pool: &sqlx::PgPool, model_id: Uuid) -> Result<()> {
    let exists = sqlx::query_scalar::<_, bool>(
        r#"
        select exists(
            select 1
            from model_definitions
            where id = $1
              and scope_kind = 'system'
        )
        "#,
    )
    .bind(model_id)
    .fetch_one(pool)
    .await?;

    if exists {
        Ok(())
    } else {
        Err(ControlPlaneError::NotFound("model_definition").into())
    }
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
