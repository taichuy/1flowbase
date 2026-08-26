use control_plane_contracts::ports::{CreateModelDefinitionInput, ModelDefinitionRepository};
use domain::DataModelScopeKind;
use storage_durable::{model_metadata::ModelMetadata, resource_descriptor::ResourceDescriptor};
use uuid::Uuid;

use crate::ordered_tree::rank::{between, rebalance, FractionalRank};
use crate::{run_migrations, PgControlPlaneStore};

mod commands;
mod prefix_indexes;
mod queries;

fn runtime_metadata(model: &domain::ModelDefinitionRecord) -> ModelMetadata {
    ModelMetadata {
        model_id: model.id,
        model_code: model.code.clone(),
        status: model.status,
        scope_kind: model.scope_kind,
        scope_id: model.scope_id,
        data_source_instance_id: model.data_source_instance_id,
        source_kind: model.source_kind,
        external_resource_key: model.external_resource_key.clone(),
        external_capability_snapshot: model
            .external_capability_snapshot
            .clone()
            .map(serde_json::from_value)
            .transpose()
            .expect("fixture capability snapshot must match the runtime contract"),
        template_provider: model.template_provider.clone(),
        template_code: model.template_code.clone(),
        template_version: model.template_version.clone(),
        physical_table_name: model.physical_table_name.clone(),
        scope_column_name: "scope_id".to_owned(),
        fields: model.fields.clone(),
        record_capabilities: domain::data_model_capabilities(model).record,
        resource: ResourceDescriptor::runtime_model(&model.code, model.scope_kind),
    }
}

fn rank(value: &str) -> FractionalRank {
    FractionalRank::parse(value).expect("fixture rank should be canonical")
}

// AC-007: normal allocation remains strictly between its supplied neighbors.
#[test]
fn fractional_rank_between_prepend_and_append_preserve_byte_order() {
    let left = rank("F");
    let right = rank("k");
    let middle = between(Some(&left), Some(&right)).unwrap().rank;
    let prepended = between(None, Some(&left)).unwrap().rank;
    let appended = between(Some(&right), None).unwrap().rank;

    assert!(prepended < left);
    assert!(left < middle);
    assert!(middle < right);
    assert!(right < appended);
    assert_eq!(
        middle.cmp(&right),
        middle.as_str().as_bytes().cmp(right.as_str().as_bytes())
    );
}

// AC-007: insertion order can repeatedly target any neighboring pair without renumbering.
#[test]
fn fractional_rank_random_insertion_matrix_stays_strict_and_canonical() {
    let mut seed = 0x6a09_e667_f3bc_c909_u64;
    let mut ranks = Vec::<FractionalRank>::new();
    for _ in 0..512 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let insertion = (seed as usize) % (ranks.len() + 1);
        let allocation = between(
            insertion.checked_sub(1).and_then(|index| ranks.get(index)),
            ranks.get(insertion),
        )
        .unwrap();
        assert!(!allocation.rank.as_str().ends_with('0'));
        ranks.insert(insertion, allocation.rank);
    }

    assert!(ranks.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(ranks
        .windows(2)
        .all(|pair| { pair[0].as_str().as_bytes() < pair[1].as_str().as_bytes() }));
}

// AC-007: rebalance is deterministic and emits a compact strictly increasing replacement batch.
#[test]
fn fractional_rank_rebalance_is_deterministic_strict_and_canonical() {
    let first = rebalance(1_024).unwrap();
    let second = rebalance(1_024).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.len(), 1_024);
    assert!(first.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(first.iter().all(|value| !value.as_str().ends_with('0')));
}

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

async fn create_workspace(store: &PgControlPlaneStore) -> Uuid {
    let tenant_id: Uuid = sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let workspace_id = Uuid::now_v7();
    sqlx::query("insert into workspaces (id, tenant_id, name) values ($1, $2, $3)")
        .bind(workspace_id)
        .bind(tenant_id)
        .bind(format!("Ordered Tree {}", workspace_id.simple()))
        .execute(store.pool())
        .await
        .unwrap();
    workspace_id
}

async fn create_ordered_tree_model(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
) -> domain::ModelDefinitionRecord {
    ModelDefinitionRepository::create_model_definition(
        store,
        &CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            template_provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.to_owned(),
            template_code: "ordered_tree".to_owned(),
            template_version: "v1".to_owned(),
            status: domain::DataModelStatus::Published,
            protection: domain::DataModelProtection::default(),
            code: format!("ordered_tree_{}", workspace_id.simple()),
            title: "Ordered Tree".to_owned(),
            description: None,
        },
    )
    .await
    .unwrap()
}

// AC-006/AC-014: the template creates its real PostgreSQL contract and protected metadata.
#[tokio::test]
async fn ordered_tree_template_creates_catalog_constraints_indexes_and_system_fields() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = create_workspace(&store).await;
    let model = create_ordered_tree_model(&store, workspace_id).await;

    let columns: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        r#"
        select column_name, data_type, is_nullable, collation_name
        from information_schema.columns
        where table_schema = current_schema() and table_name = $1
        order by ordinal_position
        "#,
    )
    .bind(&model.physical_table_name)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(columns.len(), 9);
    assert!(columns.iter().any(|column| {
        column.0 == "tree_partition_id" && column.1 == "uuid" && column.2 == "NO"
    }));
    assert!(columns
        .iter()
        .any(|column| { column.0 == "parent_id" && column.1 == "uuid" && column.2 == "YES" }));
    assert!(columns.iter().any(|column| {
        column.0 == "sibling_rank"
            && column.1 == "text"
            && column.2 == "NO"
            && column.3.as_deref() == Some("C")
    }));

    let model_uuid = model.id.simple().to_string();
    let constraints: Vec<(String, String)> = sqlx::query_as(
        r#"
        select conname, pg_get_constraintdef(oid)
        from pg_constraint
        where conrelid = (quote_ident(current_schema()) || '.' || quote_ident($1))::regclass
          and position($2 in conname) > 0
        order by conname
        "#,
    )
    .bind(&model.physical_table_name)
    .bind(&model_uuid)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        constraints,
        vec![
            (
                format!("ck_ot_parent_self_{model_uuid}"),
                "CHECK (((parent_id IS NULL) OR (parent_id <> id)))".to_owned(),
            ),
            (
                format!("fk_ot_parent_{model_uuid}"),
                format!(
                    "FOREIGN KEY (scope_id, tree_partition_id, parent_id) REFERENCES {}(scope_id, tree_partition_id, id) ON DELETE RESTRICT",
                    model.physical_table_name
                ),
            ),
            (
                format!("pk_ot_{model_uuid}"),
                "PRIMARY KEY (id)".to_owned(),
            ),
            (
                format!("uq_ot_scope_id_{model_uuid}"),
                "UNIQUE (scope_id, tree_partition_id, id)".to_owned(),
            ),
        ]
    );

    let indexes: Vec<(String, String)> = sqlx::query_as(
        r#"
        select indexname, indexdef
        from pg_indexes
        where schemaname = current_schema()
          and tablename = $1
          and position($2 in indexname) > 0
        order by indexname
        "#,
    )
    .bind(&model.physical_table_name)
    .bind(&model_uuid)
    .fetch_all(store.pool())
    .await
    .unwrap();
    let index_names = indexes
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        index_names,
        vec![
            format!("idx_ot_siblings_{model_uuid}"),
            format!("pk_ot_{model_uuid}"),
            format!("uq_ot_root_rank_{model_uuid}"),
            format!("uq_ot_scope_id_{model_uuid}"),
            format!("uq_ot_sibling_{model_uuid}"),
        ]
    );
    let index_definition = |name: &str| {
        indexes
            .iter()
            .find(|(index_name, _)| index_name == name)
            .map(|(_, definition)| definition.as_str())
            .unwrap()
    };
    assert!(index_definition(&format!("pk_ot_{model_uuid}")).contains("UNIQUE INDEX"));
    assert!(index_definition(&format!("pk_ot_{model_uuid}")).contains("(id)"));
    assert!(index_definition(&format!("uq_ot_scope_id_{model_uuid}")).contains("UNIQUE INDEX"));
    assert!(index_definition(&format!("uq_ot_scope_id_{model_uuid}"))
        .contains("(scope_id, tree_partition_id, id)"));
    assert!(index_definition(&format!("idx_ot_siblings_{model_uuid}"))
        .contains("(scope_id, tree_partition_id, parent_id, sibling_rank, id)"));
    assert!(index_definition(&format!("uq_ot_sibling_{model_uuid}")).contains("UNIQUE INDEX"));
    assert!(
        index_definition(&format!("uq_ot_sibling_{model_uuid}")).contains(
            "(scope_id, tree_partition_id, parent_id, sibling_rank) WHERE (parent_id IS NOT NULL)"
        )
    );
    assert!(index_definition(&format!("uq_ot_root_rank_{model_uuid}")).contains("UNIQUE INDEX"));
    assert!(index_definition(&format!("uq_ot_root_rank_{model_uuid}"))
        .contains("(scope_id, tree_partition_id, sibling_rank) WHERE (parent_id IS NULL)"));

    for code in ["tree_partition_id", "parent_id", "sibling_rank"] {
        let field = model
            .fields
            .iter()
            .find(|field| field.code == code)
            .expect("ordered-tree system field metadata should exist");
        assert!(field.is_system);
        assert!(!field.is_writable);
    }
    assert!(!model
        .fields
        .iter()
        .any(|field| { matches!(field.code.as_str(), "depth" | "path" | "has_children") }));
}
