use super::*;
use control_plane_contracts::ports::ModelProviderRepository;

const TABLE: &str = "model_provider_session_bindings";

async fn binding_rows(db: &PgPool) -> Vec<serde_json::Value> {
    sqlx::query_scalar(
        "select to_jsonb(binding) from model_provider_session_bindings binding \
         order by workspace_id, scope_digest",
    )
    .fetch_all(db)
    .await
    .unwrap()
}

#[tokio::test]
async fn selective_provider_data_restores_session_owners_and_preserves_target_only_bindings() {
    let (db, _) = fixture().await;
    let store = crate::PgControlPlaneStore::new(db.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Session backup")
        .await
        .unwrap();
    let scope = "a".repeat(64);
    // A historical owner remains meaningful even when its instance no longer exists.
    let original_owner = Uuid::now_v7();
    store
        .bind_model_provider_session(workspace.id, &scope, original_owner)
        .await
        .unwrap();
    let original_rows = binding_rows(&db).await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let catalog = repo.catalog().await.unwrap();
    let providers = catalog
        .iter()
        .find(|entry| entry.feature_id == "system.model-providers")
        .unwrap();
    assert!(providers.data_tables.contains(&TABLE.to_owned()));
    assert!(!providers.structure_tables.contains(&TABLE.to_owned()));
    assert!(!inventory::excluded(TABLE));

    let structure = capture(&repo, select("model-providers", true, false)).await;
    let mut scratch = archive::unpack(reader(structure.clone())).await.unwrap();
    let header = archive::header(&mut scratch.reader().await.unwrap())
        .await
        .unwrap();
    assert!(!header.tables.iter().any(|table| table.name == TABLE));
    let data = capture(&repo, select("model-providers", false, true)).await;
    scratch = archive::unpack(reader(data.clone())).await.unwrap();
    let header = archive::header(&mut scratch.reader().await.unwrap())
        .await
        .unwrap();
    let entry = header
        .tables
        .iter()
        .find(|table| table.name == TABLE)
        .unwrap();
    assert_eq!(entry.feature_id, "system.model-providers");
    assert!(entry.data);
    let preview = repo
        .preflight(reader(data.clone()), "source", "target")
        .await
        .unwrap();
    assert!(preview.failures.is_empty(), "{:?}", preview.failures);
    assert!(preview.selected_tables.contains(&TABLE.to_owned()));

    let target_owner = Uuid::now_v7();
    sqlx::query("update model_provider_session_bindings set provider_instance_id=$3 where workspace_id=$1 and scope_digest=$2")
        .bind(workspace.id).bind(&scope).bind(target_owner).execute(&db).await.unwrap();
    let target_scope = "b".repeat(64);
    store
        .bind_model_provider_session(workspace.id, &target_scope, target_owner)
        .await
        .unwrap();
    let target_rows = binding_rows(&db).await;
    repo.restore(reader(structure), "source", "target", true)
        .await
        .unwrap();
    assert_eq!(
        binding_rows(&db).await,
        target_rows,
        "configuration-only restore must not touch session facts"
    );
    let pending = repo
        .prepare_restore(reader(data.clone()), "source", "target", true)
        .await
        .unwrap();
    pending.rollback().await.unwrap();
    assert_eq!(
        binding_rows(&db).await,
        target_rows,
        "rollback must retain target owners and creation times"
    );
    repo.restore(reader(data), "source", "target", true)
        .await
        .unwrap();
    let restored_rows = binding_rows(&db).await;
    assert_eq!(restored_rows.len(), 2);
    assert_eq!(
        restored_rows[0], original_rows[0],
        "selected binding and its creation time must roundtrip exactly"
    );
    assert_eq!(
        restored_rows[1], target_rows[1],
        "target-only binding must survive restore"
    );
    assert_eq!(
        store
            .bind_model_provider_session(workspace.id, &scope, target_owner)
            .await
            .unwrap(),
        original_owner
    );
}
