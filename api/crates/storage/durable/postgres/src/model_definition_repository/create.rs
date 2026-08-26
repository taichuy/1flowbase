use anyhow::{anyhow, Result};
use control_plane::{errors::ControlPlaneError, ports::CreateModelDefinitionInput};
use uuid::Uuid;

use crate::{
    physical_schema_repository::create_runtime_model_table, repositories::PgControlPlaneStore,
};

use super::{
    change_log::{append_change_log, append_change_log_tx, ChangeLogEntry},
    field_queries::insert_model_field,
    model_queries::{insert_model_definition, insert_model_definition_after_failure},
    naming::{
        build_physical_table_name, is_registered_system_table, nullable_actor_user_id,
        registered_system_table_name,
    },
    platform_runtime_field_records,
};

const RUNTIME_TABLE_NAME_GENERATION_ATTEMPTS: usize = 8;

fn is_runtime_table_name_conflict(error: &anyhow::Error) -> bool {
    let Some(sqlx::Error::Database(database_error)) = error.downcast_ref::<sqlx::Error>() else {
        return false;
    };

    database_error.constraint() == Some("model_definitions_physical_table_name_key")
        || database_error.code().as_deref() == Some("42P07")
}

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

pub(super) async fn create_model_definition(
    store: &PgControlPlaneStore,
    input: &CreateModelDefinitionInput,
) -> Result<domain::ModelDefinitionRecord> {
    ensure_workspace_data_source_belongs_to_scope(store, input).await?;

    let registered_table_name = registered_system_table_name(
        input.scope_kind,
        input.source_kind,
        &input.protection,
        &input.code,
    );
    let retry_generated_name =
        registered_table_name.is_none() && store.runtime_table_name_policy.auto_prefix_enabled();
    let generation_attempts = if retry_generated_name {
        RUNTIME_TABLE_NAME_GENERATION_ATTEMPTS
    } else {
        1
    };

    let mut model = domain::ModelDefinitionRecord {
        id: Uuid::now_v7(),
        scope_kind: input.scope_kind,
        scope_id: input.scope_id,
        data_source_instance_id: input.data_source_instance_id,
        source_kind: input.source_kind,
        external_resource_key: input.external_resource_key.clone(),
        external_table_id: input.external_table_id.clone(),
        external_capability_snapshot: input.external_capability_snapshot.clone(),
        template_provider: input.template_provider.clone(),
        template_code: input.template_code.clone(),
        template_version: input.template_version.clone(),
        code: input.code.clone(),
        title: input.title.clone(),
        description: input.description.clone(),
        physical_table_name: match registered_table_name {
            Some(table_name) => table_name.to_string(),
            None => build_physical_table_name(&store.runtime_table_name_policy, &input.code)?,
        },
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
        model.fields = platform_runtime_field_records(&model);
    }
    let before_snapshot = serde_json::json!({});
    let actor_user_id = nullable_actor_user_id(input.actor_user_id);

    for attempt in 1..=generation_attempts {
        if attempt > 1 {
            model.physical_table_name =
                build_physical_table_name(&store.runtime_table_name_policy, &input.code)?;
        }
        let after_snapshot = serde_json::to_value(&model)?;
        let mut tx = store.pool().begin().await?;

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
                return Ok(model);
            }
            Err(error) if retry_generated_name && is_runtime_table_name_conflict(&error) => {
                tx.rollback().await?;
                if attempt < generation_attempts {
                    continue;
                }
                let exhausted_error = anyhow!(
                    "failed to generate a unique runtime physical table name after {generation_attempts} attempts"
                );
                append_change_log(
                    store.pool(),
                    &ChangeLogEntry {
                        data_model_id: None,
                        action: "model.created",
                        target_type: "model_definition",
                        target_id: Some(model.id),
                        actor_user_id,
                        before_snapshot,
                        after_snapshot,
                        execution_status: "failed",
                        error_message: Some(exhausted_error.to_string()),
                    },
                )
                .await?;
                return Err(exhausted_error);
            }
            Err(error) => {
                tx.rollback().await?;
                insert_model_definition_after_failure(
                    store.pool(),
                    &model,
                    actor_user_id,
                    domain::MetadataAvailabilityStatus::Broken,
                )
                .await?;
                append_change_log(
                    store.pool(),
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
                return Err(error);
            }
        }
    }

    Err(anyhow!(
        "runtime table name generation finished without a result"
    ))
}
