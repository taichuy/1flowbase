use control_plane_contracts::ports::{CreateModelDefinitionInput, ModelDefinitionRepository};
use domain::{DataModelProtection, DataModelScopeKind, DataModelSourceKind, DataModelStatus};

use super::{group_field_rows, load_fields_by_model_id, load_fields_for_model};
use crate::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

#[tokio::test]
async fn scoped_field_reads_preserve_values_and_order_with_measured_row_volume() {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let pool = postgres_test_support::PostgresTestSchema::create(&database_url)
        .await
        .unwrap()
        .connect()
        .await
        .unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "requested")
        .await
        .unwrap();
    let foreign_workspace = store.upsert_workspace(tenant.id, "foreign").await.unwrap();
    let mut models = Vec::new();
    for (code, scope_kind, scope_id) in [
        (
            "requested_model",
            DataModelScopeKind::Workspace,
            workspace.id,
        ),
        (
            "foreign_model",
            DataModelScopeKind::Workspace,
            foreign_workspace.id,
        ),
        (
            "global_model",
            DataModelScopeKind::System,
            domain::SYSTEM_SCOPE_ID,
        ),
        ("empty_model", DataModelScopeKind::Workspace, workspace.id),
    ] {
        let model = ModelDefinitionRepository::create_model_definition(
            &store,
            &CreateModelDefinitionInput {
                bound_storage_id: None,
                actor_user_id: Uuid::nil(),
                scope_kind,
                scope_id,
                data_source_instance_id: None,
                source_kind: DataModelSourceKind::MainSource,
                external_resource_key: None,
                external_table_id: None,
                external_capability_snapshot: None,
                template_provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.into(),
                template_code: domain::GENERAL_DATA_MODEL_TEMPLATE_CODE.into(),
                template_version: domain::GENERAL_DATA_MODEL_TEMPLATE_VERSION.into(),
                code: code.into(),
                title: code.into(),
                description: Some("Scoped field read fixture".into()),
                status: DataModelStatus::Published,
                protection: DataModelProtection::default(),
            },
        )
        .await
        .unwrap();
        models.push(model);
    }
    let [requested, foreign, global, empty] = models.as_slice() else {
        unreachable!("fixture creates four models");
    };
    sqlx::query("delete from model_fields where data_model_id = $1")
        .bind(empty.id)
        .execute(store.pool())
        .await
        .unwrap();
    // Equal sort orders must still use created_at; payloads exercise nullable/config decoding.
    for (index, field) in requested.fields.iter().enumerate() {
        sqlx::query(
            "update model_fields set sort_order = $2, created_at = '2026-01-01'::timestamptz + $3::integer * interval '1 second', display_options = $4, description = $5 where id = $1",
        )
        .bind(field.id)
        .bind((index / 2) as i32)
        .bind((requested.fields.len() - index) as i32)
        .bind(serde_json::json!({"fixture": index}))
        .bind((index % 2 == 0).then_some("field description"))
        .execute(store.pool())
        .await
        .unwrap();
    }

    // Execute the old all-model read and the production scoped loader in this same database.
    let all_rows = sqlx::query(
        "select * from model_fields order by data_model_id asc, sort_order asc, created_at asc",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    let total_rows = all_rows.len();
    let all_fields = group_field_rows(all_rows);
    let requested_ids = [requested.id, global.id, empty.id, Uuid::now_v7()];
    let fields = load_fields_by_model_id(store.pool(), &requested_ids)
        .await
        .unwrap();
    let scoped_rows = fields.values().map(Vec::len).sum::<usize>();
    assert!(
        total_rows > scoped_rows,
        "foreign fields must not be decoded by the scoped loader"
    );
    assert_eq!(fields.len(), 2);
    assert!(!fields.contains_key(&foreign.id));
    for model_id in [requested.id, global.id] {
        assert_eq!(fields[&model_id], all_fields[&model_id]);
    }
    let single_fields = load_fields_for_model(store.pool(), requested.id)
        .await
        .unwrap();
    assert_eq!(single_fields, fields[&requested.id]);
    let expected_order = requested
        .fields
        .chunks(2)
        .flat_map(|pair| pair.iter().rev().map(|field| field.id))
        .collect::<Vec<_>>();
    assert_eq!(
        single_fields
            .iter()
            .map(|field| field.id)
            .collect::<Vec<_>>(),
        expected_order
    );
    println!(
        "model_field_row_volume F_total={total_rows} F_requested={scoped_rows} F_single={} output_equivalent=true",
        single_fields.len()
    );

    let visible = ModelDefinitionRepository::list_model_definitions(&store, workspace.id)
        .await
        .unwrap();
    assert!(visible.iter().any(|model| model.id == requested.id));
    assert!(visible.iter().any(|model| model.id == global.id));
    assert!(!visible.iter().any(|model| model.id == foreign.id));
    assert!(visible
        .iter()
        .find(|model| model.id == empty.id)
        .unwrap()
        .fields
        .is_empty());
    for model in &visible {
        assert_eq!(
            model.fields,
            all_fields.get(&model.id).cloned().unwrap_or_default()
        );
    }
    let all_models = ModelDefinitionRepository::list_model_definitions(&store, Uuid::nil())
        .await
        .unwrap();
    assert!(all_models.iter().any(|model| model.id == foreign.id));
    assert!(
        ModelDefinitionRepository::get_model_definition(&store, workspace.id, foreign.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        ModelDefinitionRepository::get_model_definition(&store, workspace.id, Uuid::now_v7())
            .await
            .unwrap()
            .is_none()
    );
    assert!(load_fields_by_model_id(store.pool(), &[])
        .await
        .unwrap()
        .is_empty());
    assert!(
        load_fields_by_model_id(store.pool(), &[empty.id, Uuid::now_v7()])
            .await
            .unwrap()
            .is_empty()
    );
    let mut tx = store.pool().begin().await.unwrap();
    let transaction_fields = load_fields_for_model(&mut *tx, requested.id).await.unwrap();
    assert_eq!(transaction_fields, single_fields);
    tx.rollback().await.unwrap();
}
