use super::*;
use std::sync::Arc;
use tokio::sync::Barrier;

#[tokio::test]
async fn provider_session_binding_has_one_concurrent_first_writer_and_survives_store_recreation() {
    let (store, workspace, _, _) = seed_store().await;
    let scope = "a".repeat(64);
    assert_eq!(
        store
            .get_model_provider_session_binding(workspace.id, &scope)
            .await
            .unwrap(),
        None
    );
    let targets = (0..12).map(|_| Uuid::now_v7()).collect::<Vec<_>>();
    let barrier = Arc::new(Barrier::new(targets.len()));
    let mut tasks = Vec::new();
    for target in &targets {
        let store = PgControlPlaneStore::new(store.pool().clone());
        let barrier = barrier.clone();
        let scope = scope.clone();
        let target = *target;
        let workspace_id = workspace.id;
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            store
                .bind_model_provider_session(workspace_id, &scope, target)
                .await
                .unwrap()
        }));
    }
    let mut owners = Vec::new();
    for task in tasks {
        owners.push(task.await.unwrap());
    }
    let owner = owners[0];
    assert!(targets.contains(&owner));
    assert!(owners.iter().all(|candidate| *candidate == owner));

    let recreated = PgControlPlaneStore::new(store.pool().clone());
    assert_eq!(
        recreated
            .get_model_provider_session_binding(workspace.id, &scope)
            .await
            .unwrap(),
        Some(owner)
    );
    assert_eq!(
        recreated
            .bind_model_provider_session(workspace.id, &scope, Uuid::now_v7())
            .await
            .unwrap(),
        owner
    );
}

#[tokio::test]
async fn provider_session_binding_is_scoped_and_does_not_disappear_with_its_instance() {
    let (store, workspace, actor, installation_id) = seed_store().await;
    let instance = create_ready_instance(
        &store,
        workspace.id,
        actor.id,
        installation_id,
        "session-origin",
        vec!["model".into()],
    )
    .await;
    let scope = "b".repeat(64);
    let other_scope = "c".repeat(64);
    assert_eq!(
        store
            .bind_model_provider_session(workspace.id, &scope, instance.id)
            .await
            .unwrap(),
        instance.id
    );
    assert_eq!(
        store
            .get_model_provider_session_binding(Uuid::now_v7(), &scope)
            .await
            .unwrap(),
        None
    );
    let other_target = Uuid::now_v7();
    assert_eq!(
        store
            .bind_model_provider_session(workspace.id, &other_scope, other_target)
            .await
            .unwrap(),
        other_target
    );
    ModelProviderRepository::delete_instance(&store, workspace.id, instance.id)
        .await
        .unwrap();
    assert_eq!(
        store
            .get_model_provider_session_binding(workspace.id, &scope)
            .await
            .unwrap(),
        Some(instance.id)
    );
    assert_eq!(
        store
            .bind_model_provider_session(workspace.id, &scope, other_target)
            .await
            .unwrap(),
        instance.id
    );
}
