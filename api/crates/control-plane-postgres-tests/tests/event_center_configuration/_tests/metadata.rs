use control_plane_contracts::ports::ModelDefinitionRepository;
use domain::{
    builtin_contract_for_model, data_model_capabilities, data_model_field_capabilities,
    SYSTEM_SCOPE_ID,
};
use serde_json::{json, Value};
use uuid::Uuid;

#[tokio::test]
async fn metadata_seed_is_protected_and_replay_preserves_user_presentation_and_grants() {
    let (store, workspace, _) = super::support::seed_store().await;
    for code in ["event_definitions", "workflow_event_triggers"] {
        let id: Uuid =
            sqlx::query_scalar("SELECT id FROM model_definitions WHERE code=$1 AND scope_id=$2")
                .bind(code)
                .bind(SYSTEM_SCOPE_ID)
                .fetch_one(store.pool())
                .await
                .unwrap();
        let model = store
            .get_model_definition(SYSTEM_SCOPE_ID, id)
            .await
            .unwrap()
            .unwrap();
        let contract = builtin_contract_for_model(&model).expect("protected builtin metadata");
        let capabilities = data_model_capabilities(&model);
        assert!(!capabilities.can_delete);
        assert!(!capabilities.can_add_user_field);
        assert!(!capabilities.record.can_create);
        assert!(!capabilities.record.can_update);
        assert!(!capabilities.record.can_delete);
        assert_eq!(model.fields.len(), contract.system_field_codes.len());
        assert!(model
            .fields
            .iter()
            .all(|field| field.code != "contract_version"));
        for field in &model.fields {
            let capabilities = data_model_field_capabilities(&model, field);
            let field_contract = contract.field_contract(&field.code).unwrap();
            assert!(!capabilities.can_update_physical_metadata);
            assert!(!capabilities.can_delete);
            assert_eq!(field.field_kind, field_contract.field_kind);
            assert_eq!(field.is_required, field_contract.is_required);
            assert!(field.is_system);
            assert!(!field.is_writable);
        }
        if code == "workflow_event_triggers" {
            let source = model
                .fields
                .iter()
                .find(|field| field.code == "source")
                .unwrap();
            assert_eq!(source.field_kind, domain::ModelFieldKind::Json);
            assert!(!source.is_required);
        }
        let initial_grants: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM scope_data_model_grants WHERE data_model_id=$1",
        )
        .bind(id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(
            initial_grants, 0,
            "configuration must not gain generic workspace access"
        );
        sqlx::query("UPDATE model_definitions SET title='User title',description='User model description' WHERE id=$1").bind(id).execute(store.pool()).await.unwrap();
        sqlx::query("UPDATE model_fields SET title='User field title',description='User field description',display_interface='input',display_options='{\"fixture\":true}'::jsonb WHERE data_model_id=$1 AND code='contract_id'").bind(id).execute(store.pool()).await.unwrap();
        sqlx::query("INSERT INTO scope_data_model_grants(id,scope_kind,scope_id,data_model_id,enabled,permission_profile) VALUES($1,'workspace',$2,$3,false,'owner')").bind(Uuid::now_v7()).bind(workspace.id).bind(id).execute(store.pool()).await.unwrap();
        let model_before: Value =
            sqlx::query_scalar("SELECT to_jsonb(m) FROM model_definitions m WHERE id=$1")
                .bind(id)
                .fetch_one(store.pool())
                .await
                .unwrap();
        let field_before: Value = sqlx::query_scalar(
            "SELECT to_jsonb(f) FROM model_fields f WHERE data_model_id=$1 AND code='contract_id'",
        )
        .bind(id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        let grant_before: Value = sqlx::query_scalar(
            "SELECT to_jsonb(g) FROM scope_data_model_grants g WHERE data_model_id=$1",
        )
        .bind(id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        // A missing metadata seed is restored, while existing rows remain byte-for-byte unchanged.
        sqlx::query(
            "DELETE FROM model_fields WHERE data_model_id=$1 AND code IN ('enabled','source')",
        )
        .bind(id)
        .execute(store.pool())
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../../../../storage/durable/postgres/migrations/20261107020000_event_center_current_metadata.sql")).execute(store.pool()).await.unwrap();
        let model_after: Value =
            sqlx::query_scalar("SELECT to_jsonb(m) FROM model_definitions m WHERE id=$1")
                .bind(id)
                .fetch_one(store.pool())
                .await
                .unwrap();
        let field_after: Value = sqlx::query_scalar(
            "SELECT to_jsonb(f) FROM model_fields f WHERE data_model_id=$1 AND code='contract_id'",
        )
        .bind(id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        let grant_after: Value = sqlx::query_scalar(
            "SELECT to_jsonb(g) FROM scope_data_model_grants g WHERE data_model_id=$1",
        )
        .bind(id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(model_before, model_after);
        assert_eq!(field_before, field_after);
        assert_eq!(grant_before, grant_after);
        assert_eq!(field_after["display_options"], json!({"fixture":true}));
        assert_eq!(
            store
                .get_model_definition(SYSTEM_SCOPE_ID, id)
                .await
                .unwrap()
                .unwrap()
                .fields
                .len(),
            contract.system_field_codes.len()
        );
    }
}
