use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
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
        OrderedTreeBoundedListInput, OrderedTreeChildrenInput, OrderedTreeCommandError,
        OrderedTreeCreateInput, OrderedTreeCreatePosition, OrderedTreeDescendantsInput,
        OrderedTreeLeafDeleteInput, OrderedTreeMoveInput, OrderedTreeMovePosition,
        OrderedTreeNodeInput, OrderedTreeQueryError, OrderedTreeQueryRepository,
        OrderedTreeSearchInput, OrderedTreeSubtreeDeleteInput, OrderedTreeSubtreeImpactInput,
    },
};
use serde_json::{json, Map, Value};
use sqlx::{postgres::PgRow, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{
    ordered_tree::commands::{
        create_ordered_tree_node_in_transaction, delete_ordered_tree_leaf_in_transaction,
        delete_ordered_tree_subtree_in_transaction, move_ordered_tree_node_in_transaction,
        snapshot_ordered_tree_subtree_in_transaction,
    },
    repositories::PgControlPlaneStore,
};

const FRONTSTAGE_BLOCK_MODEL_ID: Uuid = Uuid::from_u128(0xe6aa0cc5_dfc0_8d8d_b6c8_b9bd0113a61a);
const FRONTSTAGE_BLOCK_TABLE: &str = "frontstage_block_nodes";
const DETAIL_COLUMNS: &str = r#"
    node.block_id,
    node.scope_id,
    node.tree_partition_id,
    node.tab_id,
    parent.block_id as parent_block_id,
    node.sibling_rank,
    node.presentation,
    node.title,
    node.description,
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
            field("description", Text, false, 50),
            field("code_ref", String, true, 60),
            field("schema_version", Number, true, 70),
            field("input_mapping", Json, true, 80),
            field("output_mapping", Json, true, 90),
            field("runtime_descriptor", Json, true, 100),
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

fn summary_field(
    code: &str,
    kind: domain::ModelFieldKind,
    required: bool,
    is_system: bool,
    sort_order: i32,
) -> domain::ModelFieldRecord {
    let mut field = field(code, kind, required, sort_order);
    field.is_system = is_system;
    field.is_writable = false;
    field
}

fn frontstage_block_summary_metadata(workspace_id: Uuid) -> ModelMetadata {
    use domain::ModelFieldKind::{Datetime, ManyToOne, Number, String, Text};

    let mut metadata = frontstage_block_metadata(workspace_id);
    metadata.fields = vec![
        summary_field("id", String, true, true, 10),
        summary_field("scope_id", ManyToOne, true, true, 20),
        summary_field("tree_partition_id", ManyToOne, true, true, 30),
        summary_field("parent_id", ManyToOne, false, true, 40),
        summary_field("sibling_rank", String, true, true, 50),
        summary_field("block_id", String, true, false, 60),
        summary_field("tab_id", ManyToOne, true, true, 70),
        summary_field("presentation", String, true, true, 80),
        summary_field("title", Text, false, false, 90),
        summary_field("description", Text, false, false, 100),
        summary_field("schema_version", Number, true, true, 110),
        summary_field("created_at", Datetime, true, true, 120),
        summary_field("updated_at", Datetime, true, true, 130),
    ];
    metadata.record_capabilities = domain::DataModelRecordCapabilities::read_only();
    metadata
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
        description: row.get("description"),
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
        description: summary.description,
        code_ref: row.get("code_ref"),
        schema_version: summary.schema_version,
        input_mapping: serde_json::from_value(input_mapping)?,
        output_mapping: serde_json::from_value(output_mapping)?,
        runtime_descriptor: row.get("runtime_descriptor"),
        created_at: summary.created_at,
        updated_at: summary.updated_at,
    })
}

fn map_runtime_layer(row: &PgRow) -> Result<domain::frontstage::FrontstageBlockRuntimeLayer> {
    Ok(domain::frontstage::FrontstageBlockRuntimeLayer {
        node: map_record(row)?,
        code: row.get("code"),
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
        select {DETAIL_COLUMNS}, node.input_mapping, node.output_mapping, node.runtime_descriptor
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

#[derive(Debug, Clone)]
struct InternalBlockSummary {
    internal_id: Uuid,
    parent_internal_id: Option<Uuid>,
    block_id: String,
    workspace_id: Uuid,
    page_id: Uuid,
    tab_id: Uuid,
    rank: String,
    presentation: domain::FrontstageBlockPresentation,
    title: Option<String>,
    description: Option<String>,
    schema_version: u32,
    created_at: time::OffsetDateTime,
    updated_at: time::OffsetDateTime,
}

impl InternalBlockSummary {
    fn into_node(
        self,
        public_ids: &HashMap<Uuid, String>,
    ) -> Result<domain::FrontstageBlockNodeSummary> {
        let block_id = required_public_id(public_ids, self.internal_id)?.clone();
        if block_id != self.block_id {
            return Err(anyhow!(
                "frontstage block summary public id does not match its internal id mapping"
            ));
        }
        let parent_block_id = self
            .parent_internal_id
            .map(|parent_id| required_public_id(public_ids, parent_id).cloned())
            .transpose()?;
        Ok(domain::FrontstageBlockNodeSummary {
            block_id,
            workspace_id: self.workspace_id,
            page_id: self.page_id,
            tab_id: self.tab_id,
            parent_block_id,
            rank: self.rank,
            presentation: self.presentation,
            title: self.title,
            description: self.description,
            schema_version: self.schema_version,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

fn record_object(record: &Value) -> Result<&Map<String, Value>> {
    record
        .as_object()
        .ok_or_else(|| anyhow!("frontstage block summary projection must be an object"))
}

fn required_string<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("frontstage block summary field `{field}` must be a string"))
}

fn required_uuid(object: &Map<String, Value>, field: &str) -> Result<Uuid> {
    Ok(Uuid::parse_str(required_string(object, field)?)?)
}

fn optional_uuid(object: &Map<String, Value>, field: &str) -> Result<Option<Uuid>> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(Uuid::parse_str(value.as_str().ok_or_else(|| {
            anyhow!("frontstage block summary field `{field}` must be a UUID or null")
        })?)?)),
    }
}

fn required_datetime(object: &Map<String, Value>, field: &str) -> Result<time::OffsetDateTime> {
    Ok(time::OffsetDateTime::parse(
        required_string(object, field)?,
        &time::format_description::well_known::Rfc3339,
    )?)
}

fn decode_summary(record: &Value) -> Result<InternalBlockSummary> {
    let object = record_object(record)?;
    let presentation = required_string(object, "presentation")?;
    let schema_version = object
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            anyhow!("frontstage block summary field `schema_version` must be an integer")
        })?
        .try_into()?;
    Ok(InternalBlockSummary {
        internal_id: required_uuid(object, "id")?,
        parent_internal_id: optional_uuid(object, "parent_id")?,
        block_id: required_string(object, "block_id")?.to_owned(),
        workspace_id: required_uuid(object, "scope_id")?,
        page_id: required_uuid(object, "tree_partition_id")?,
        tab_id: required_uuid(object, "tab_id")?,
        rank: required_string(object, "sibling_rank")?.to_owned(),
        presentation: domain::FrontstageBlockPresentation::from_db(presentation).ok_or(
            ControlPlaneError::InvalidInput("frontstage_block_presentation"),
        )?,
        title: object
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_owned),
        description: object
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_owned),
        schema_version,
        created_at: required_datetime(object, "created_at")?,
        updated_at: required_datetime(object, "updated_at")?,
    })
}

fn required_public_id(public_ids: &HashMap<Uuid, String>, internal_id: Uuid) -> Result<&String> {
    public_ids.get(&internal_id).ok_or_else(|| {
        anyhow!("frontstage block summary references unmapped internal id `{internal_id}`")
    })
}

async fn map_public_ids(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    page_id: Uuid,
    internal_ids: HashSet<Uuid>,
) -> Result<HashMap<Uuid, String>> {
    if internal_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        select id, block_id
        from frontstage_block_nodes
        where scope_id = $1 and tree_partition_id = $2 and id = any($3)
        "#,
    )
    .bind(workspace_id)
    .bind(page_id)
    .bind(internal_ids.iter().copied().collect::<Vec<_>>())
    .fetch_all(store.pool())
    .await?;
    let public_ids = rows.into_iter().collect::<HashMap<_, _>>();
    for internal_id in internal_ids {
        required_public_id(&public_ids, internal_id)?;
    }
    Ok(public_ids)
}

async fn map_node_records(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    page_id: Uuid,
    records: impl IntoIterator<Item = Value>,
) -> Result<Vec<domain::FrontstageBlockNodeSummary>> {
    let summaries = records
        .into_iter()
        .map(|record| decode_summary(&record))
        .collect::<Result<Vec<_>>>()?;
    let public_ids = map_public_ids(
        store,
        workspace_id,
        page_id,
        summaries
            .iter()
            .flat_map(|summary| [Some(summary.internal_id), summary.parent_internal_id])
            .flatten()
            .collect(),
    )
    .await?;
    summaries
        .into_iter()
        .map(|summary| summary.into_node(&public_ids))
        .collect()
}

async fn resolve_query_node_id(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    page_id: Uuid,
    block_id: &str,
    missing: OrderedTreeQueryError,
) -> Result<Uuid> {
    sqlx::query_scalar(
        "select id from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2 and block_id = $3",
    )
    .bind(workspace_id)
    .bind(page_id)
    .bind(block_id)
    .fetch_optional(store.pool())
    .await?
    .ok_or_else(|| anyhow::Error::new(missing))
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
        if let Some(description) = &input.description {
            payload.insert("description".to_owned(), json!(description));
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
            select {DETAIL_COLUMNS}, node.input_mapping, node.output_mapping, node.runtime_descriptor
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

    async fn get_frontstage_block_runtime_assembly(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
    ) -> Result<Vec<domain::frontstage::FrontstageBlockRuntimeLayer>> {
        // A single bounded recursive read owns ancestor ordering and source resolution so
        // runtime callers never fan out into one descriptor/code query per tree layer.
        let rows = sqlx::query(&format!(
            r#"
            with recursive ancestry as (
                select target.id, target.parent_id, 0::integer as depth, array[target.id] as path
                from frontstage_block_nodes target
                where target.scope_id = $1
                  and target.tree_partition_id = $2
                  and target.block_id = $3

                union all

                select parent_node.id,
                       parent_node.parent_id,
                       ancestry.depth + 1,
                       ancestry.path || parent_node.id
                from ancestry
                join frontstage_block_nodes parent_node
                  on parent_node.scope_id = $1
                 and parent_node.tree_partition_id = $2
                 and parent_node.id = ancestry.parent_id
                where ancestry.depth < 63
                  and not parent_node.id = any(ancestry.path)
            )
            select {DETAIL_COLUMNS}, node.input_mapping, node.output_mapping,
                   node.runtime_descriptor, source.code
            from ancestry
            join frontstage_block_nodes node
              on node.scope_id = $1
             and node.tree_partition_id = $2
             and node.id = ancestry.id
            left join frontstage_block_nodes parent
              on parent.scope_id = node.scope_id
             and parent.tree_partition_id = node.tree_partition_id
             and parent.id = node.parent_id
            join frontstage_block_codes source
              on source.workspace_id = node.scope_id
             and source.page_id = node.tree_partition_id
             and source.code_ref = node.code_ref
            order by ancestry.depth desc
            "#,
        ))
        .bind(workspace_id)
        .bind(page_id)
        .bind(block_id)
        .fetch_all(self.pool())
        .await?;
        rows.iter().map(map_runtime_layer).collect()
    }

    async fn list_frontstage_block_roots(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        limit: u32,
    ) -> Result<Vec<domain::FrontstageBlockNodeSummary>> {
        let metadata = frontstage_block_summary_metadata(workspace_id);
        let projections = OrderedTreeQueryRepository::list_ordered_tree_roots(
            self,
            &metadata,
            OrderedTreeBoundedListInput {
                scope_id: workspace_id,
                tree_partition_id: page_id,
                result_limit: limit,
            },
        )
        .await?;
        map_node_records(
            self,
            workspace_id,
            page_id,
            projections.into_iter().map(|projection| projection.record),
        )
        .await
    }

    async fn list_frontstage_block_children(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        parent_block_id: &str,
        limit: u32,
    ) -> Result<Vec<domain::FrontstageBlockNodeSummary>> {
        let parent_id = resolve_query_node_id(
            self,
            workspace_id,
            page_id,
            parent_block_id,
            OrderedTreeQueryError::ParentNotFound,
        )
        .await?;
        let metadata = frontstage_block_summary_metadata(workspace_id);
        let projections = OrderedTreeQueryRepository::list_ordered_tree_children(
            self,
            &metadata,
            OrderedTreeChildrenInput {
                scope_id: workspace_id,
                tree_partition_id: page_id,
                parent_id,
                result_limit: limit,
            },
        )
        .await?;
        map_node_records(
            self,
            workspace_id,
            page_id,
            projections.into_iter().map(|projection| projection.record),
        )
        .await
    }

    async fn list_frontstage_block_ancestors(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
    ) -> Result<Vec<domain::FrontstageBlockNodeSummary>> {
        let node_id = resolve_query_node_id(
            self,
            workspace_id,
            page_id,
            block_id,
            OrderedTreeQueryError::NodeNotFound,
        )
        .await?;
        let metadata = frontstage_block_summary_metadata(workspace_id);
        let projections = OrderedTreeQueryRepository::list_ordered_tree_ancestors(
            self,
            &metadata,
            OrderedTreeNodeInput {
                scope_id: workspace_id,
                tree_partition_id: page_id,
                node_id,
            },
        )
        .await?;
        map_node_records(
            self,
            workspace_id,
            page_id,
            projections.into_iter().map(|projection| projection.record),
        )
        .await
    }

    async fn list_frontstage_block_descendants(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
        max_depth: u32,
        limit: u32,
    ) -> Result<Vec<domain::FrontstageBlockDescendantProjection>> {
        let node_id = resolve_query_node_id(
            self,
            workspace_id,
            page_id,
            block_id,
            OrderedTreeQueryError::NodeNotFound,
        )
        .await?;
        let metadata = frontstage_block_summary_metadata(workspace_id);
        let projections = OrderedTreeQueryRepository::list_ordered_tree_descendants(
            self,
            &metadata,
            OrderedTreeDescendantsInput {
                scope_id: workspace_id,
                tree_partition_id: page_id,
                node_id,
                max_depth,
                result_limit: limit,
                include_path: true,
            },
        )
        .await?;
        let summaries = projections
            .iter()
            .map(|projection| decode_summary(&projection.record))
            .collect::<Result<Vec<_>>>()?;
        let public_ids = map_public_ids(
            self,
            workspace_id,
            page_id,
            summaries
                .iter()
                .flat_map(|summary| [Some(summary.internal_id), summary.parent_internal_id])
                .flatten()
                .chain(
                    projections
                        .iter()
                        .flat_map(|projection| projection.path.iter().flatten().copied()),
                )
                .collect(),
        )
        .await?;
        projections
            .into_iter()
            .zip(summaries)
            .map(|(projection, summary)| {
                let path = projection
                    .path
                    .ok_or_else(|| anyhow!("ordered-tree descendant path was not projected"))?
                    .into_iter()
                    .map(|internal_id| required_public_id(&public_ids, internal_id).cloned())
                    .collect::<Result<Vec<_>>>()?;
                Ok(domain::FrontstageBlockDescendantProjection {
                    node: summary.into_node(&public_ids)?,
                    depth: projection.depth,
                    has_children: projection.has_children,
                    path,
                })
            })
            .collect()
    }

    async fn search_frontstage_blocks(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        query: &str,
        limit: u32,
    ) -> Result<Vec<domain::FrontstageBlockSearchResult>> {
        let metadata = frontstage_block_summary_metadata(workspace_id);
        let projections = OrderedTreeQueryRepository::search_ordered_tree_prefix(
            self,
            &metadata,
            OrderedTreeSearchInput {
                scope_id: workspace_id,
                tree_partition_id: page_id,
                prefix: query.to_owned(),
                match_limit: limit,
            },
        )
        .await?;
        let summaries = projections
            .iter()
            .map(|projection| decode_summary(&projection.record))
            .collect::<Result<Vec<_>>>()?;
        let public_ids = map_public_ids(
            self,
            workspace_id,
            page_id,
            summaries
                .iter()
                .flat_map(|summary| [Some(summary.internal_id), summary.parent_internal_id])
                .flatten()
                .collect(),
        )
        .await?;
        let by_internal_id = summaries
            .iter()
            .cloned()
            .map(|summary| (summary.internal_id, summary))
            .collect::<HashMap<_, _>>();
        projections
            .into_iter()
            .zip(summaries)
            .filter(|(projection, _)| projection.is_match)
            .map(|(_, summary)| {
                let mut ancestors = Vec::new();
                let mut current = summary.parent_internal_id;
                let mut visited = HashSet::new();
                while let Some(internal_id) = current {
                    if !visited.insert(internal_id) {
                        return Err(anyhow!(
                            "frontstage block search ancestor context contains a cycle"
                        ));
                    }
                    let ancestor = by_internal_id.get(&internal_id).ok_or_else(|| {
                        anyhow!(
                            "frontstage block search is missing projected ancestor `{internal_id}`"
                        )
                    })?;
                    current = ancestor.parent_internal_id;
                    ancestors.push(ancestor.clone().into_node(&public_ids)?);
                }
                ancestors.reverse();
                Ok(domain::FrontstageBlockSearchResult {
                    node: summary.into_node(&public_ids)?,
                    ancestors,
                })
            })
            .collect()
    }

    async fn get_frontstage_block_subtree_impact(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
    ) -> Result<domain::FrontstageBlockSubtreeImpact> {
        let node_id = resolve_query_node_id(
            self,
            workspace_id,
            page_id,
            block_id,
            OrderedTreeQueryError::NodeNotFound,
        )
        .await?;
        let metadata = frontstage_block_summary_metadata(workspace_id);
        let result = OrderedTreeQueryRepository::get_ordered_tree_subtree_impact(
            self,
            &metadata,
            OrderedTreeSubtreeImpactInput {
                scope_id: workspace_id,
                tree_partition_id: page_id,
                node_id,
            },
        )
        .await?;
        Ok(domain::FrontstageBlockSubtreeImpact {
            affected_count: result.affected_count,
        })
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
                description = case when $8 then $9 else description end,
                input_mapping = case when $10 then $11 else input_mapping end,
                output_mapping = case when $12 then $13 else output_mapping end,
                runtime_descriptor = case when $14 then $15 else runtime_descriptor end,
                updated_by = $16,
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
        .bind(input.description.is_some())
        .bind(input.description.clone().flatten())
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
        let node_id = sqlx::query_scalar(
            "select id from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2 and block_id = $3",
        )
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(&input.block_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| anyhow::Error::new(OrderedTreeCommandError::NodeNotFound))?;
        let snapshot = snapshot_ordered_tree_subtree_in_transaction(
            &mut tx,
            &metadata,
            input.workspace_id,
            input.page_id,
            node_id,
        )
        .await?;
        let code_refs: Vec<String> = sqlx::query_scalar(
            r#"
            select code_ref
            from frontstage_block_nodes
            where scope_id = $1 and tree_partition_id = $2 and id = any($3)
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(snapshot.node_ids_child_before_parent())
        .fetch_all(&mut *tx)
        .await?;
        if code_refs.len() as u64 != snapshot.affected_count() {
            return Err(anyhow!(
                "frontstage block subtree code snapshot does not match the locked structure"
            ));
        }
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
        let deleted_codes = sqlx::query(
            "delete from frontstage_block_codes where workspace_id = $1 and page_id = $2 and code_ref = any($3)",
        )
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(code_refs)
        .execute(&mut *tx)
        .await?;
        if deleted_codes.rows_affected() != deleted.deleted_count {
            return Err(anyhow!(
                "frontstage block subtree code cleanup does not match deleted nodes"
            ));
        }
        insert_audit(&mut tx, &input.audit_log).await?;
        tx.commit().await?;
        Ok(FrontstageBlockSubtreeDeleteResult {
            deleted_count: deleted.deleted_count,
        })
    }
}
