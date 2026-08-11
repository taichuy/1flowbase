use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        CreateFrontstageBlockNodeInput, DeleteFrontstageBlockLeafInput,
        DeleteFrontstageBlockSubtreeInput, FrontstageBlockPosition,
        FrontstageBlockSubtreeDeleteResult, FrontstageBlockTreeRepository,
        MoveFrontstageBlockNodeInput, SaveFrontstageBlockNodeCodeInput,
        UpdateFrontstageBlockNodeInput,
    },
};
use runtime_core::{
    model_metadata::ModelMetadata,
    resource_descriptor::{
        Exposure, Plane, ResourceDescriptor, ResourceKind, TenantScope, TrustLevel,
    },
    runtime_record_repository::{
        OrderedTreeCommandError, OrderedTreeCreateInput, OrderedTreeCreatePosition,
        OrderedTreeLeafDeleteInput, OrderedTreeMoveInput, OrderedTreeMovePosition,
        OrderedTreeSubtreeDeleteInput,
    },
};
use serde_json::{json, Map, Value};
use sqlx::{postgres::PgRow, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{
    ordered_tree::commands::{
        create_ordered_tree_node_in_transaction, delete_ordered_tree_leaf_in_transaction,
        delete_ordered_tree_subtree_in_transaction, move_ordered_tree_node_in_transaction,
    },
    repositories::PgControlPlaneStore,
};

const FRONTSTAGE_BLOCK_MODEL_ID: Uuid = Uuid::from_u128(0xe6aa0cc5_dfc0_8d8d_b6c8_b9bd0113a61a);
const FRONTSTAGE_BLOCK_TABLE: &str = "frontstage_block_nodes";
const SUMMARY_COLUMNS: &str = r#"
    node.block_id,
    node.scope_id,
    node.tree_partition_id,
    node.tab_id,
    parent.block_id as parent_block_id,
    node.sibling_rank,
    node.presentation,
    node.title,
    node.code_ref,
    node.schema_version,
    node.created_at,
    node.updated_at
"#;

fn field(
    code: &str,
    kind: domain::ModelFieldKind,
    required: bool,
    sort_order: i32,
) -> domain::ModelFieldRecord {
    domain::ModelFieldRecord {
        id: Uuid::nil(),
        data_model_id: FRONTSTAGE_BLOCK_MODEL_ID,
        code: code.to_owned(),
        title: code.to_owned(),
        description: None,
        physical_column_name: code.to_owned(),
        external_field_key: None,
        field_kind: kind,
        is_system: false,
        is_writable: true,
        is_required: required,
        api_required: required,
        is_unique: matches!(code, "block_id" | "code_ref"),
        default_value: None,
        display_interface: None,
        display_options: json!({}),
        relation_target_model_id: None,
        relation_options: json!({}),
        sort_order,
        availability_status: domain::MetadataAvailabilityStatus::Available,
    }
}

fn frontstage_block_metadata(workspace_id: Uuid) -> ModelMetadata {
    use domain::ModelFieldKind::{Json, ManyToOne, Number, String, Text};

    ModelMetadata {
        model_id: FRONTSTAGE_BLOCK_MODEL_ID,
        model_code: "frontstage_block_nodes".to_owned(),
        status: domain::DataModelStatus::Published,
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: workspace_id,
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        external_capability_snapshot: None,
        template_provider: "core".to_owned(),
        template_code: "ordered_tree".to_owned(),
        template_version: "v1".to_owned(),
        physical_table_name: FRONTSTAGE_BLOCK_TABLE.to_owned(),
        scope_column_name: "scope_id".to_owned(),
        fields: vec![
            field("block_id", String, true, 10),
            field("tab_id", ManyToOne, true, 20),
            field("presentation", String, true, 30),
            field("title", Text, false, 40),
            field("code_ref", String, true, 50),
            field("schema_version", Number, true, 60),
            field("input_mapping", Json, true, 70),
            field("output_mapping", Json, true, 80),
            field("runtime_descriptor", Json, true, 90),
        ],
        record_capabilities: domain::DataModelRecordCapabilities::read_write(),
        resource: ResourceDescriptor::new(
            "internal.frontstage.block_nodes",
            ResourceKind::Static,
            Plane::Internal,
            Exposure::Internal,
            TenantScope::Workspace,
            TrustLevel::Core,
        ),
    }
}

fn validate_audit(workspace_id: Uuid, audit: &domain::AuditLogRecord) -> Result<()> {
    if audit.workspace_id != Some(workspace_id) || audit.actor_user_id.is_none() {
        return Err(ControlPlaneError::InvalidInput("frontstage_block_audit_scope").into());
    }
    Ok(())
}

fn validate_actor_audit(
    workspace_id: Uuid,
    actor_user_id: Uuid,
    audit: &domain::AuditLogRecord,
) -> Result<()> {
    validate_audit(workspace_id, audit)?;
    if audit.actor_user_id != Some(actor_user_id) {
        return Err(ControlPlaneError::InvalidInput("frontstage_block_audit_actor").into());
    }
    Ok(())
}

fn validate_create(input: &CreateFrontstageBlockNodeInput) -> Result<()> {
    validate_actor_audit(input.workspace_id, input.actor_user_id, &input.audit_log)?;
    if input.block_id.trim().is_empty() || input.code_ref.trim().is_empty() {
        return Err(ControlPlaneError::InvalidInput("frontstage_block_identity").into());
    }
    if input.schema_version != 1 {
        return Err(ControlPlaneError::InvalidInput("frontstage_block_schema_version").into());
    }
    if !input.runtime_descriptor.is_object() {
        return Err(ControlPlaneError::InvalidInput("frontstage_block_runtime_descriptor").into());
    }
    Ok(())
}

async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    audit: &domain::AuditLogRecord,
) -> Result<()> {
    let scope_id = audit.workspace_id.unwrap_or(domain::SYSTEM_SCOPE_ID);
    sqlx::query(
        r#"
        insert into audit_logs (
            id, workspace_id, scope_id, actor_user_id, target_type, target_id,
            event_code, payload, created_by, updated_by, created_at, updated_at
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, $4, $4, $9, $9)
        "#,
    )
    .bind(audit.id)
    .bind(audit.workspace_id)
    .bind(scope_id)
    .bind(audit.actor_user_id)
    .bind(&audit.target_type)
    .bind(audit.target_id)
    .bind(&audit.event_code)
    .bind(&audit.payload)
    .bind(audit.created_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn internal_id(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    page_id: Uuid,
    block_id: &str,
) -> Result<Option<Uuid>> {
    Ok(sqlx::query_scalar(
        "select id from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2 and block_id = $3 for update",
    )
    .bind(workspace_id)
    .bind(page_id)
    .bind(block_id)
    .fetch_optional(&mut **tx)
    .await?)
}

async fn required_internal_id(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    page_id: Uuid,
    block_id: &str,
    error: OrderedTreeCommandError,
) -> Result<Uuid> {
    internal_id(tx, workspace_id, page_id, block_id)
        .await?
        .ok_or_else(|| anyhow::Error::new(error))
}

async fn required_internal_id_in_tab(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    page_id: Uuid,
    tab_id: Uuid,
    block_id: &str,
    error: OrderedTreeCommandError,
) -> Result<Uuid> {
    sqlx::query_scalar(
        "select id from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2 and tab_id = $3 and block_id = $4 for update",
    )
    .bind(workspace_id)
    .bind(page_id)
    .bind(tab_id)
    .bind(block_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| anyhow::Error::new(error))
}

async fn resolve_position(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    page_id: Uuid,
    tab_id: Uuid,
    position: &FrontstageBlockPosition,
) -> Result<OrderedTreeCreatePosition> {
    let parent_id = match &position.parent_block_id {
        Some(value) => Some(
            required_internal_id_in_tab(
                tx,
                workspace_id,
                page_id,
                tab_id,
                value,
                OrderedTreeCommandError::ParentNotFound,
            )
            .await?,
        ),
        None => None,
    };
    let before_id = match &position.before_block_id {
        Some(value) => Some(
            required_internal_id_in_tab(
                tx,
                workspace_id,
                page_id,
                tab_id,
                value,
                OrderedTreeCommandError::AnchorNotFound,
            )
            .await?,
        ),
        None => None,
    };
    let after_id = match &position.after_block_id {
        Some(value) => Some(
            required_internal_id_in_tab(
                tx,
                workspace_id,
                page_id,
                tab_id,
                value,
                OrderedTreeCommandError::AnchorNotFound,
            )
            .await?,
        ),
        None => None,
    };
    Ok(OrderedTreeCreatePosition {
        parent_id,
        before_id,
        after_id,
    })
}

fn map_summary(row: &PgRow) -> Result<domain::FrontstageBlockNodeSummary> {
    let presentation: String = row.get("presentation");
    let schema_version: i64 = row.get("schema_version");
    Ok(domain::FrontstageBlockNodeSummary {
        block_id: row.get("block_id"),
        workspace_id: row.get("scope_id"),
        page_id: row.get("tree_partition_id"),
        tab_id: row.get("tab_id"),
        parent_block_id: row.get("parent_block_id"),
        rank: row.get("sibling_rank"),
        presentation: domain::FrontstageBlockPresentation::from_db(&presentation).ok_or(
            ControlPlaneError::InvalidInput("frontstage_block_presentation"),
        )?,
        title: row.get("title"),
        code_ref: row.get("code_ref"),
        schema_version: schema_version
            .try_into()
            .map_err(|_| ControlPlaneError::InvalidInput("frontstage_block_schema_version"))?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_record(row: &PgRow) -> Result<domain::FrontstageBlockNodeRecord> {
    let summary = map_summary(row)?;
    let input_mapping: Value = row.get("input_mapping");
    let output_mapping: Value = row.get("output_mapping");
    Ok(domain::FrontstageBlockNodeRecord {
        block_id: summary.block_id,
        workspace_id: summary.workspace_id,
        page_id: summary.page_id,
        tab_id: summary.tab_id,
        parent_block_id: summary.parent_block_id,
        rank: summary.rank,
        presentation: summary.presentation,
        title: summary.title,
        code_ref: summary.code_ref,
        schema_version: summary.schema_version,
        input_mapping: serde_json::from_value(input_mapping)?,
        output_mapping: serde_json::from_value(output_mapping)?,
        runtime_descriptor: row.get("runtime_descriptor"),
        created_at: summary.created_at,
        updated_at: summary.updated_at,
    })
}

async fn get_record_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    page_id: Uuid,
    block_id: &str,
) -> Result<Option<domain::FrontstageBlockNodeRecord>> {
    let row = sqlx::query(&format!(
        r#"
        select {SUMMARY_COLUMNS}, node.input_mapping, node.output_mapping, node.runtime_descriptor
        from frontstage_block_nodes node
        left join frontstage_block_nodes parent
          on parent.scope_id = node.scope_id
         and parent.tree_partition_id = node.tree_partition_id
         and parent.id = node.parent_id
        where node.scope_id = $1 and node.tree_partition_id = $2 and node.block_id = $3
        "#,
    ))
    .bind(workspace_id)
    .bind(page_id)
    .bind(block_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.as_ref().map(map_record).transpose()
}

fn bounded_limit(limit: i64) -> i64 {
    limit.clamp(1, 1000)
}

#[async_trait]
impl FrontstageBlockTreeRepository for PgControlPlaneStore {
    async fn create_frontstage_block_node(
        &self,
        input: &CreateFrontstageBlockNodeInput,
    ) -> Result<domain::FrontstageBlockNodeRecord> {
        validate_create(input)?;
        let metadata = frontstage_block_metadata(input.workspace_id);
        let mut tx = self.pool().begin().await?;
        let position = resolve_position(
            &mut tx,
            input.workspace_id,
            input.page_id,
            input.tab_id,
            &input.position,
        )
        .await?;

        let inserted_code = sqlx::query_scalar::<_, Uuid>(
            r#"
            insert into frontstage_block_codes (
                id, workspace_id, page_id, code_ref, code, created_by, updated_by
            ) values ($1, $2, $3, $4, $5, $6, $6)
            on conflict (workspace_id, page_id, code_ref) do nothing
            returning id
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(&input.code_ref)
        .bind(&input.code)
        .bind(input.actor_user_id)
        .fetch_optional(&mut *tx)
        .await?;
        if inserted_code.is_none() {
            return Err(ControlPlaneError::Conflict("frontstage_block_code_exists").into());
        }

        let mut payload = Map::from_iter([
            ("block_id".to_owned(), json!(input.block_id)),
            ("tab_id".to_owned(), json!(input.tab_id)),
            (
                "presentation".to_owned(),
                json!(input.presentation.as_str()),
            ),
            ("code_ref".to_owned(), json!(input.code_ref)),
            (
                "input_mapping".to_owned(),
                serde_json::to_value(&input.input_mapping)?,
            ),
            (
                "output_mapping".to_owned(),
                serde_json::to_value(&input.output_mapping)?,
            ),
            (
                "runtime_descriptor".to_owned(),
                input.runtime_descriptor.clone(),
            ),
        ]);
        if let Some(title) = &input.title {
            payload.insert("title".to_owned(), json!(title));
        }
        create_ordered_tree_node_in_transaction(
            &mut tx,
            &metadata,
            OrderedTreeCreateInput {
                actor_user_id: input.actor_user_id,
                scope_id: input.workspace_id,
                tree_partition_id: input.page_id,
                payload: Value::Object(payload),
                position,
            },
        )
        .await?;
        insert_audit(&mut tx, &input.audit_log).await?;
        let record =
            get_record_in_transaction(&mut tx, input.workspace_id, input.page_id, &input.block_id)
                .await?
                .ok_or(ControlPlaneError::NotFound("frontstage_block_node"))?;
        tx.commit().await?;
        Ok(record)
    }

    async fn get_frontstage_block_node(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
    ) -> Result<Option<domain::FrontstageBlockNodeRecord>> {
        let row = sqlx::query(&format!(
            r#"
            select {SUMMARY_COLUMNS}, node.input_mapping, node.output_mapping, node.runtime_descriptor
            from frontstage_block_nodes node
            left join frontstage_block_nodes parent
              on parent.scope_id = node.scope_id
             and parent.tree_partition_id = node.tree_partition_id
             and parent.id = node.parent_id
            where node.scope_id = $1 and node.tree_partition_id = $2 and node.block_id = $3
            "#,
        ))
        .bind(workspace_id)
        .bind(page_id)
        .bind(block_id)
        .fetch_optional(self.pool())
        .await?;
        row.as_ref().map(map_record).transpose()
    }

    async fn list_frontstage_block_roots(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        limit: i64,
    ) -> Result<Vec<domain::FrontstageBlockNodeSummary>> {
        let rows = sqlx::query(&format!(
            r#"select {SUMMARY_COLUMNS}
               from frontstage_block_nodes node
               left join frontstage_block_nodes parent on false
               where node.scope_id = $1 and node.tree_partition_id = $2 and node.parent_id is null
               order by node.sibling_rank collate "C", node.id limit $3"#,
        ))
        .bind(workspace_id)
        .bind(page_id)
        .bind(bounded_limit(limit))
        .fetch_all(self.pool())
        .await?;
        rows.iter().map(map_summary).collect()
    }

    async fn list_frontstage_block_children(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        parent_block_id: &str,
        limit: i64,
    ) -> Result<Vec<domain::FrontstageBlockNodeSummary>> {
        let rows = sqlx::query(&format!(
            r#"select {SUMMARY_COLUMNS}
               from frontstage_block_nodes node
               join frontstage_block_nodes parent
                 on parent.scope_id = node.scope_id and parent.tree_partition_id = node.tree_partition_id
                and parent.id = node.parent_id
               where node.scope_id = $1 and node.tree_partition_id = $2 and parent.block_id = $3
               order by node.sibling_rank collate "C", node.id limit $4"#,
        ))
        .bind(workspace_id)
        .bind(page_id)
        .bind(parent_block_id)
        .bind(bounded_limit(limit))
        .fetch_all(self.pool())
        .await?;
        rows.iter().map(map_summary).collect()
    }

    async fn list_frontstage_block_ancestors(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
    ) -> Result<Vec<domain::FrontstageBlockNodeSummary>> {
        let rows = sqlx::query(&format!(
            r#"
            with recursive ancestors(id, depth, path) as (
                select node.parent_id, 1, array[node.id]
                from frontstage_block_nodes node
                where node.scope_id = $1 and node.tree_partition_id = $2 and node.block_id = $3
                union all
                select parent.parent_id, ancestors.depth + 1, ancestors.path || parent.id
                from ancestors
                join frontstage_block_nodes parent
                  on parent.scope_id = $1 and parent.tree_partition_id = $2 and parent.id = ancestors.id
                where ancestors.id is not null and not parent.id = any(ancestors.path) and ancestors.depth < 256
            )
            select {SUMMARY_COLUMNS}
            from ancestors
            join frontstage_block_nodes node
              on node.scope_id = $1 and node.tree_partition_id = $2 and node.id = ancestors.id
            left join frontstage_block_nodes parent
              on parent.scope_id = node.scope_id and parent.tree_partition_id = node.tree_partition_id
             and parent.id = node.parent_id
            order by ancestors.depth desc
            "#,
        ))
        .bind(workspace_id)
        .bind(page_id)
        .bind(block_id)
        .fetch_all(self.pool())
        .await?;
        rows.iter().map(map_summary).collect()
    }

    async fn list_frontstage_block_descendants(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
        max_depth: i32,
        limit: i64,
    ) -> Result<Vec<domain::FrontstageBlockNodeSummary>> {
        let rows = sqlx::query(&format!(
            r#"
            with recursive descendants(id, depth, path) as (
                select node.id, 0, array[node.id]
                from frontstage_block_nodes node
                where node.scope_id = $1 and node.tree_partition_id = $2 and node.block_id = $3
                union all
                select child.id, descendants.depth + 1, descendants.path || child.id
                from descendants
                join frontstage_block_nodes child
                  on child.scope_id = $1 and child.tree_partition_id = $2
                 and child.parent_id = descendants.id
                where descendants.depth < $4 and not child.id = any(descendants.path)
            )
            select {SUMMARY_COLUMNS}
            from descendants
            join frontstage_block_nodes node
              on node.scope_id = $1 and node.tree_partition_id = $2 and node.id = descendants.id
            left join frontstage_block_nodes parent
              on parent.scope_id = node.scope_id and parent.tree_partition_id = node.tree_partition_id
             and parent.id = node.parent_id
            where descendants.depth > 0
            order by descendants.depth, node.sibling_rank collate "C", node.id
            limit $5
            "#,
        ))
        .bind(workspace_id)
        .bind(page_id)
        .bind(block_id)
        .bind(max_depth.clamp(1, 256))
        .bind(bounded_limit(limit))
        .fetch_all(self.pool())
        .await?;
        rows.iter().map(map_summary).collect()
    }

    async fn search_frontstage_blocks(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        query: &str,
        limit: i64,
    ) -> Result<Vec<domain::FrontstageBlockNodeSummary>> {
        let rows = sqlx::query(&format!(
            r#"select {SUMMARY_COLUMNS}
               from frontstage_block_nodes node
               left join frontstage_block_nodes parent
                 on parent.scope_id = node.scope_id and parent.tree_partition_id = node.tree_partition_id
                and parent.id = node.parent_id
               where node.scope_id = $1 and node.tree_partition_id = $2
                 and (node.block_id ilike $3 || '%' or coalesce(node.title, '') ilike '%' || $3 || '%')
               order by node.sibling_rank collate "C", node.id limit $4"#,
        ))
        .bind(workspace_id)
        .bind(page_id)
        .bind(query)
        .bind(bounded_limit(limit).min(100))
        .fetch_all(self.pool())
        .await?;
        rows.iter().map(map_summary).collect()
    }

    async fn update_frontstage_block_node(
        &self,
        input: &UpdateFrontstageBlockNodeInput,
    ) -> Result<domain::FrontstageBlockNodeRecord> {
        validate_actor_audit(input.workspace_id, input.actor_user_id, &input.audit_log)?;
        if input
            .runtime_descriptor
            .as_ref()
            .is_some_and(|value| !value.is_object())
        {
            return Err(
                ControlPlaneError::InvalidInput("frontstage_block_runtime_descriptor").into(),
            );
        }
        let mut tx = self.pool().begin().await?;
        let row = sqlx::query(
            r#"
            update frontstage_block_nodes
            set presentation = case when $4 then $5 else presentation end,
                title = case when $6 then $7 else title end,
                input_mapping = case when $8 then $9 else input_mapping end,
                output_mapping = case when $10 then $11 else output_mapping end,
                runtime_descriptor = case when $12 then $13 else runtime_descriptor end,
                updated_by = $14,
                updated_at = now()
            where scope_id = $1 and tree_partition_id = $2 and block_id = $3
            returning id
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(&input.block_id)
        .bind(input.presentation.is_some())
        .bind(input.presentation.map(|value| value.as_str()))
        .bind(input.title.is_some())
        .bind(input.title.clone().flatten())
        .bind(input.input_mapping.is_some())
        .bind(
            input
                .input_mapping
                .as_ref()
                .map(serde_json::to_value)
                .transpose()?,
        )
        .bind(input.output_mapping.is_some())
        .bind(
            input
                .output_mapping
                .as_ref()
                .map(serde_json::to_value)
                .transpose()?,
        )
        .bind(input.runtime_descriptor.is_some())
        .bind(input.runtime_descriptor.clone())
        .bind(input.actor_user_id)
        .fetch_optional(&mut *tx)
        .await?;
        if row.is_none() {
            return Err(ControlPlaneError::NotFound("frontstage_block_node").into());
        }
        insert_audit(&mut tx, &input.audit_log).await?;
        let record =
            get_record_in_transaction(&mut tx, input.workspace_id, input.page_id, &input.block_id)
                .await?
                .ok_or(ControlPlaneError::NotFound("frontstage_block_node"))?;
        tx.commit().await?;
        Ok(record)
    }

    async fn save_frontstage_block_node_code(
        &self,
        input: &SaveFrontstageBlockNodeCodeInput,
    ) -> Result<domain::frontstage::FrontstageBlockCodeRecord> {
        validate_actor_audit(input.workspace_id, input.actor_user_id, &input.audit_log)?;
        let mut tx = self.pool().begin().await?;
        let code_ref: Option<String> = sqlx::query_scalar(
            "select code_ref from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2 and block_id = $3 for update",
        )
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(&input.block_id)
        .fetch_optional(&mut *tx)
        .await?;
        let code_ref = code_ref.ok_or(ControlPlaneError::NotFound("frontstage_block_node"))?;
        let row = sqlx::query(
            r#"
            update frontstage_block_codes
            set code = $4, updated_by = $5, updated_at = now()
            where workspace_id = $1 and page_id = $2 and code_ref = $3
            returning workspace_id, page_id, code_ref, code, created_at, updated_at
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(&code_ref)
        .bind(&input.code)
        .bind(input.actor_user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ControlPlaneError::NotFound("frontstage_block_code"))?;
        insert_audit(&mut tx, &input.audit_log).await?;
        tx.commit().await?;
        Ok(domain::frontstage::FrontstageBlockCodeRecord {
            workspace_id: row.get("workspace_id"),
            page_id: row.get("page_id"),
            code_ref: row.get("code_ref"),
            code: row.get("code"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn move_frontstage_block_node(
        &self,
        input: &MoveFrontstageBlockNodeInput,
    ) -> Result<domain::FrontstageBlockNodeRecord> {
        validate_actor_audit(input.workspace_id, input.actor_user_id, &input.audit_log)?;
        let metadata = frontstage_block_metadata(input.workspace_id);
        let mut tx = self.pool().begin().await?;
        let (node_id, tab_id): (Uuid, Uuid) = sqlx::query_as(
            "select id, tab_id from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2 and block_id = $3 for update",
        )
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(&input.block_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| anyhow::Error::new(OrderedTreeCommandError::NodeNotFound))?;
        let position = resolve_position(
            &mut tx,
            input.workspace_id,
            input.page_id,
            tab_id,
            &input.position,
        )
        .await?;
        move_ordered_tree_node_in_transaction(
            &mut tx,
            &metadata,
            OrderedTreeMoveInput {
                actor_user_id: input.actor_user_id,
                scope_id: input.workspace_id,
                tree_partition_id: input.page_id,
                node_id,
                position: OrderedTreeMovePosition {
                    new_parent_id: position.parent_id,
                    before_id: position.before_id,
                    after_id: position.after_id,
                },
            },
        )
        .await?;
        insert_audit(&mut tx, &input.audit_log).await?;
        let record =
            get_record_in_transaction(&mut tx, input.workspace_id, input.page_id, &input.block_id)
                .await?
                .ok_or(ControlPlaneError::NotFound("frontstage_block_node"))?;
        tx.commit().await?;
        Ok(record)
    }

    async fn delete_frontstage_block_leaf(
        &self,
        input: &DeleteFrontstageBlockLeafInput,
    ) -> Result<bool> {
        validate_audit(input.workspace_id, &input.audit_log)?;
        let metadata = frontstage_block_metadata(input.workspace_id);
        let mut tx = self.pool().begin().await?;
        let Some((node_id, code_ref)) = sqlx::query_as::<_, (Uuid, String)>(
            "select id, code_ref from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2 and block_id = $3 for update",
        )
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(&input.block_id)
        .fetch_optional(&mut *tx)
        .await?
        else {
            tx.commit().await?;
            return Ok(false);
        };
        let deleted = delete_ordered_tree_leaf_in_transaction(
            &mut tx,
            &metadata,
            OrderedTreeLeafDeleteInput {
                scope_id: input.workspace_id,
                tree_partition_id: input.page_id,
                node_id,
            },
        )
        .await?;
        if deleted {
            sqlx::query(
                "delete from frontstage_block_codes where workspace_id = $1 and page_id = $2 and code_ref = $3",
            )
            .bind(input.workspace_id)
            .bind(input.page_id)
            .bind(code_ref)
            .execute(&mut *tx)
            .await?;
            insert_audit(&mut tx, &input.audit_log).await?;
        }
        tx.commit().await?;
        Ok(deleted)
    }

    async fn delete_frontstage_block_subtree(
        &self,
        input: &DeleteFrontstageBlockSubtreeInput,
    ) -> Result<FrontstageBlockSubtreeDeleteResult> {
        validate_audit(input.workspace_id, &input.audit_log)?;
        let metadata = frontstage_block_metadata(input.workspace_id);
        let mut tx = self.pool().begin().await?;
        let node_id = required_internal_id(
            &mut tx,
            input.workspace_id,
            input.page_id,
            &input.block_id,
            OrderedTreeCommandError::NodeNotFound,
        )
        .await?;
        let code_refs: Vec<String> = sqlx::query_scalar(
            r#"
            with recursive subtree(id, path) as (
                select id, array[id]
                from frontstage_block_nodes
                where scope_id = $1 and tree_partition_id = $2 and id = $3
                union all
                select child.id, subtree.path || child.id
                from subtree
                join frontstage_block_nodes child
                  on child.scope_id = $1 and child.tree_partition_id = $2 and child.parent_id = subtree.id
                where not child.id = any(subtree.path)
            )
            select node.code_ref from subtree join frontstage_block_nodes node on node.id = subtree.id
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(node_id)
        .fetch_all(&mut *tx)
        .await?;
        let deleted = delete_ordered_tree_subtree_in_transaction(
            &mut tx,
            &metadata,
            OrderedTreeSubtreeDeleteInput {
                scope_id: input.workspace_id,
                tree_partition_id: input.page_id,
                node_id,
                expected_affected_count: input.expected_affected_count,
            },
        )
        .await?;
        sqlx::query(
            "delete from frontstage_block_codes where workspace_id = $1 and page_id = $2 and code_ref = any($3)",
        )
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(code_refs)
        .execute(&mut *tx)
        .await?;
        insert_audit(&mut tx, &input.audit_log).await?;
        tx.commit().await?;
        Ok(FrontstageBlockSubtreeDeleteResult {
            deleted_count: deleted.deleted_count,
        })
    }
}
