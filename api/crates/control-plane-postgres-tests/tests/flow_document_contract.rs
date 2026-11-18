mod support;

use control_plane::flow::{FlowService, SaveFlowDraftCommand};
use control_plane_contracts::ports::{
    ApplicationRepository, CreateApplicationInput, FlowRepository,
};
use domain::{ApplicationType, FlowChangeKind};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn missing_version_restore_does_not_bootstrap_editor_state() {
    let (store, workspace, actor) = support::seed_store().await;
    let app = ApplicationRepository::create_application(
        &store,
        &CreateApplicationInput {
            actor_user_id: actor.id,
            workspace_id: workspace.id,
            application_type: ApplicationType::Workflow,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Restore boundary".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        },
    )
    .await
    .unwrap();
    let count = || {
        sqlx::query_scalar::<_, i64>("select count(*) from flows where application_id = $1")
            .bind(app.id)
            .fetch_one(store.pool())
    };
    assert_eq!(count().await.unwrap(), 0);
    let error = FlowService::new(store.clone())
        .restore_version(actor.id, app.id, Uuid::now_v7())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("flow_version"));
    assert_eq!(count().await.unwrap(), 0);
}

#[tokio::test]
async fn version_reads_are_scoped_and_invalid_writes_preserve_postgres_history() {
    let (store, workspace, actor) = support::seed_store().await;
    let app = ApplicationRepository::create_application(
        &store,
        &CreateApplicationInput {
            actor_user_id: actor.id,
            workspace_id: workspace.id,
            application_type: ApplicationType::Workflow,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Write boundary".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        },
    )
    .await
    .unwrap();
    let service = FlowService::new(store.clone());
    let before = service
        .get_or_create_editor_state(actor.id, app.id)
        .await
        .unwrap();
    let version_id = before.versions[0].id;
    assert_eq!(
        FlowRepository::load_flow_version_document(&store, workspace.id, app.id, version_id)
            .await
            .unwrap(),
        Some(before.draft.document.clone())
    );
    assert!(
        FlowRepository::load_flow_version_document(&store, Uuid::now_v7(), app.id, version_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(FlowRepository::load_flow_version_document(
        &store,
        workspace.id,
        Uuid::now_v7(),
        version_id
    )
    .await
    .unwrap()
    .is_none());
    let mut invalid = before.draft.document.clone();
    invalid["graph"]["nodes"][0]["outputs"] = json!([{"key":"duplicate"}]);
    service
        .save_draft(SaveFlowDraftCommand {
            actor_user_id: actor.id,
            application_id: app.id,
            document: invalid,
            change_kind: FlowChangeKind::Logical,
            summary: "must not persist".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(
        service
            .get_or_create_editor_state(actor.id, app.id)
            .await
            .unwrap(),
        before
    );
}
