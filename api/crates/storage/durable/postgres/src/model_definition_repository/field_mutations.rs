use super::*;

impl PgControlPlaneStore {
    pub(super) async fn add_model_field_impl(
        &self,
        input: &AddModelFieldInput,
    ) -> Result<domain::ModelFieldRecord> {
        if input.field_kind == domain::ModelFieldKind::FilePreviewUrl
            && (input.code != "preview_url"
                || !input.is_system
                || input.is_writable
                || input.is_required
                || input.api_required
                || input.is_unique
                || input.default_value.is_some())
        {
            return Err(ControlPlaneError::InvalidInput("computed_field_definition").into());
        }
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
        if crate::ordered_tree::schema::matches(&model)
            && (input.code == "tree_path" || physical_column_name == "tree_path")
        {
            return Err(ControlPlaneError::InvalidInput("reserved ordered-tree field").into());
        }
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
                    domain::ModelFieldKind::OneToMany | domain::ModelFieldKind::FilePreviewUrl => {}
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

    pub(super) async fn update_model_field_impl(
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

    pub(super) async fn reconcile_system_model_field_impl(
        &self,
        input: &ReconcileSystemModelFieldInput,
    ) -> Result<domain::ModelFieldRecord> {
        if input.physical_column_name == "tree_path" {
            let ordered_tree: bool = sqlx::query_scalar("select exists(select 1 from model_definitions where id = $1 and template_provider = 'core' and template_code = 'ordered_tree' and template_version = 'v1')")
                .bind(input.model_id).fetch_one(self.pool()).await?;
            if ordered_tree {
                return Err(ControlPlaneError::InvalidInput("reserved ordered-tree field").into());
            }
        }
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
}
