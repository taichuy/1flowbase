use control_plane::ports::{CreateModelDefinitionInput, ModelDefinitionRepository};
use domain::DataModelScopeKind;
use uuid::Uuid;

use super::rank::{between, rebalance, FractionalRank};
use crate::{run_migrations, PgControlPlaneStore};

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
    assert_eq!(columns.len(), 8);
    assert!(columns
        .iter()
        .any(|column| { column.0 == "parent_id" && column.1 == "uuid" && column.2 == "YES" }));
    assert!(columns.iter().any(|column| {
        column.0 == "sibling_rank"
            && column.1 == "text"
            && column.2 == "NO"
            && column.3.as_deref() == Some("C")
    }));

    let constraints: Vec<(String, String)> = sqlx::query_as(
        r#"
        select conname, pg_get_constraintdef(oid)
        from pg_constraint
        where conrelid = (quote_ident(current_schema()) || '.' || quote_ident($1))::regclass
        "#,
    )
    .bind(&model.physical_table_name)
    .fetch_all(store.pool())
    .await
    .unwrap();
    let model_uuid = model.id.simple().to_string();
    assert!(constraints.iter().all(|(name, _)| name.len() <= 63));
    assert!(constraints
        .iter()
        .all(|(name, _)| name.contains(&model_uuid)));
    assert!(constraints
        .iter()
        .any(|(_, definition)| { definition == "UNIQUE (scope_id, id)" }));
    assert!(constraints.iter().any(|definition| {
        definition
            .1
            .contains("CHECK (((parent_id IS NULL) OR (parent_id <> id)))")
    }));
    assert!(constraints.iter().any(|definition| {
        definition.1.contains("FOREIGN KEY (scope_id, parent_id)")
            && definition.1.contains("REFERENCES")
            && definition.1.contains("(scope_id, id) ON DELETE RESTRICT")
    }));

    let indexes: Vec<(String, String)> = sqlx::query_as(
        r#"
        select indexname, indexdef
        from pg_indexes
        where schemaname = current_schema() and tablename = $1
        "#,
    )
    .bind(&model.physical_table_name)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert!(indexes.iter().all(|(name, _)| name.len() <= 63));
    assert!(indexes.iter().all(|(name, _)| name.contains(&model_uuid)));
    assert!(indexes
        .iter()
        .any(|(_, definition)| { definition.contains("(scope_id, parent_id, sibling_rank, id)") }));
    assert!(indexes.iter().any(|(_, definition)| {
        definition.contains("UNIQUE")
            && definition.contains("(scope_id, parent_id, sibling_rank)")
            && definition.contains("WHERE (parent_id IS NOT NULL)")
    }));
    assert!(indexes.iter().any(|(_, definition)| {
        definition.contains("UNIQUE")
            && definition.contains("(scope_id, sibling_rank)")
            && definition.contains("WHERE (parent_id IS NULL)")
    }));

    for code in ["parent_id", "sibling_rank"] {
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
