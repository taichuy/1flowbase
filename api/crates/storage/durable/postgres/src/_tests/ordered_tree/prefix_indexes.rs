use std::borrow::Cow;

use control_plane::ports::{AddModelFieldInput, ModelDefinitionRepository};
use domain::ModelFieldKind;
use serde_json::json;
use sqlx::migrate::Migrator;
use storage_durable::runtime_record_repository::RuntimeRecordRepository;
use uuid::Uuid;

use super::{create_ordered_tree_model, create_workspace, isolated_database, runtime_metadata};
use crate::{run_migrations, PgControlPlaneStore};

const DROP_TEXT_PREFIX_INDEXES_MIGRATION_VERSION: i64 = 20260811150000;

fn before_drop_text_prefix_indexes_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < DROP_TEXT_PREFIX_INDEXES_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

async fn add_text_field(store: &PgControlPlaneStore, model_id: Uuid) -> domain::ModelFieldRecord {
    ModelDefinitionRepository::add_model_field(
        store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id,
            external_field_key: None,
            code: "content".to_owned(),
            title: "Content".to_owned(),
            description: None,
            field_kind: ModelFieldKind::Text,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: false,
            api_required: false,
            is_unique: false,
            default_value: None,
            display_interface: Some("textarea".to_owned()),
            display_options: json!({}),
            relation_target_model_id: None,
            physical_column_name: None,
            relation_options: json!({}),
        },
    )
    .await
    .unwrap()
}

async fn index_exists(store: &PgControlPlaneStore, index_name: &str) -> bool {
    sqlx::query_scalar(
        "select exists(select 1 from pg_indexes where schemaname = current_schema() and indexname = $1)",
    )
    .bind(index_name)
    .fetch_one(store.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn ordered_tree_text_field_accepts_long_runtime_content_without_prefix_index() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope_id = create_workspace(&store).await;
    let model = create_ordered_tree_model(&store, scope_id).await;
    let field = add_text_field(&store, model.id).await;
    let model = ModelDefinitionRepository::get_model_definition(&store, Uuid::nil(), model.id)
        .await
        .unwrap()
        .unwrap();
    let long_markdown = format!(
        "# Long architecture chapter\n\n{}",
        "正文段落。".repeat(4_096)
    );
    assert!(long_markdown.len() > 13_696);

    let created = RuntimeRecordRepository::create_record(
        &store,
        &runtime_metadata(&model),
        Uuid::nil(),
        scope_id,
        json!({
            "tree_partition_id": scope_id,
            "sibling_rank": "U",
            "content": long_markdown,
        }),
    )
    .await
    .unwrap();

    assert_eq!(created["content"], long_markdown);
    let index_name = format!("idx_ot_prefix_{}", field.id.simple());
    assert!(!index_exists(&store, &index_name).await);
}

#[tokio::test]
async fn migration_drops_legacy_ordered_tree_text_prefix_indexes() {
    let pool = isolated_database().await.connect().await.unwrap();
    before_drop_text_prefix_indexes_migrator()
        .run(&pool)
        .await
        .unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let scope_id = create_workspace(&store).await;
    let model = create_ordered_tree_model(&store, scope_id).await;
    let field = add_text_field(&store, model.id).await;
    let index_name = format!("idx_ot_prefix_{}", field.id.simple());

    sqlx::query(&format!(
        "create index if not exists \"{index_name}\" on \"{}\" (scope_id, tree_partition_id, (lower(\"{}\") collate \"C\") text_pattern_ops)",
        model.physical_table_name, field.physical_column_name
    ))
    .execute(store.pool())
    .await
    .unwrap();
    assert!(index_exists(&store, &index_name).await);

    run_migrations(&pool).await.unwrap();

    assert!(!index_exists(&store, &index_name).await);
}
