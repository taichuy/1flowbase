use control_plane::{
    application_public_api::run_service::{
        ApplicationPublishedFlowRunRepository, CreateAssistantConversationInput,
        ListAssistantConversationsInput,
    },
    ports::{ApplicationRepository, CreateApplicationInput},
};
use domain::ApplicationType;

use super::*;

#[tokio::test]
async fn assistant_conversation_keeps_an_explicit_read_only_legacy_snapshot_seed() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-08-06 08:00:00 UTC);
    let legacy_run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "legacy-assistant-snapshot".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::AssistantExecution,
            target_node_id: None,
            title: "Legacy refund question".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({
                "node-start": {
                    "query": "What is the refund policy?\n\n<page_references trust=\"untrusted\">...</page_references>"
                },
                "__embedded_assistant_user_message": {
                    "content": "What is the refund policy?",
                    "page_references": [{
                        "page_url": "http://console.test/refunds",
                        "page_title": "Refunds",
                        "outer_html": "<div id=\"refunds\">Seven days</div>"
                    }]
                }
            }),
            started_at,
            api_key_id: None,
            publication_version_id: None,
            assistant_conversation_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: Some("embedded_assistant".to_string()),
            idempotency_key: None,
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: legacy_run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "Refunds are available within seven days." }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(1)),
        },
    )
    .await
    .unwrap();

    let conversation_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into assistant_conversations (
            conversation_id,
            scope_id,
            application_id,
            created_by,
            seed_legacy_flow_run_id
        ) values ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(conversation_id)
    .bind(seeded.workspace_id)
    .bind(seeded.application_id)
    .bind(seeded.actor_user_id)
    .bind(legacy_run.id)
    .execute(store.pool())
    .await
    .unwrap();

    let messages = <PgControlPlaneStore as ApplicationPublishedFlowRunRepository>::list_assistant_conversation_messages(
        &store,
        seeded.workspace_id,
        seeded.application_id,
        seeded.actor_user_id,
        conversation_id,
    )
    .await
    .unwrap();
    assert_eq!(
        messages
            .iter()
            .map(|message| (message.role.as_str(), message.content.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("user", "What is the refund policy?"),
            ("assistant", "Refunds are available within seven days.")
        ]
    );
    assert_eq!(messages[0].page_references.len(), 1);
    assert_eq!(
        messages[0].page_references[0].outer_html(),
        "<div id=\"refunds\">Seven days</div>"
    );
    assert!(messages[1].page_references.is_empty());
    assert!(messages.iter().all(|message| message.status == "succeeded"));

    let legacy_snapshot = <PgControlPlaneStore as ApplicationPublishedFlowRunRepository>::list_assistant_legacy_snapshot_messages(
        &store,
        seeded.workspace_id,
        seeded.application_id,
        seeded.actor_user_id,
        legacy_run.id,
    )
    .await
    .unwrap();
    assert_eq!(legacy_snapshot.len(), 2);
    assert_eq!(legacy_snapshot[0].page_references.len(), 1);
}

/// BE-001 AC-003: a cancelled Assistant run exposes its canonical partial answer as one cancelled
/// assistant message; an empty cancellation output does not manufacture an assistant message.
#[tokio::test]
async fn assistant_conversation_projects_cancelled_partial_answer_status() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let conversation = <PgControlPlaneStore as ApplicationPublishedFlowRunRepository>::create_assistant_conversation(
        &store,
        &CreateAssistantConversationInput {
            conversation_id: Uuid::now_v7(),
            workspace_id: seeded.workspace_id,
            application_id: seeded.application_id,
            actor_user_id: seeded.actor_user_id,
            seed_legacy_flow_run_id: None,
        },
    )
    .await
    .unwrap();

    for (index, output_payload) in [
        json!({ "answer": "partial answer", "__canonical_answer_presentation": true }),
        json!({}),
    ]
    .into_iter()
    .enumerate()
    {
        let run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
            &store,
            &CreateFlowRunInput {
                actor_user_id: seeded.actor_user_id,
                application_id: seeded.application_id,
                flow_id: seeded.flow_id,
                flow_draft_id: seeded.draft_id,
                compiled_plan_id: compiled.id,
                debug_session_id: format!("cancelled-assistant-{index}"),
                flow_schema_version: compiled.schema_version.clone(),
                document_hash: compiled.document_hash.clone(),
                run_mode: FlowRunMode::AssistantExecution,
                target_node_id: None,
                title: format!("question {index}"),
                status: FlowRunStatus::Running,
                input_payload: json!({ "node-start": { "query": format!("question {index}") } }),
                started_at: OffsetDateTime::now_utc(),
                api_key_id: None,
                publication_version_id: None,
                assistant_conversation_id: Some(conversation.conversation_id),
                external_user: None,
                external_conversation_id: None,
                external_trace_id: None,
                compatibility_mode: Some("embedded_assistant".to_string()),
                idempotency_key: None,
            },
        )
        .await
        .unwrap();
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::commit_flow_run_terminal(
            &store,
            &CommitFlowRunTerminalInput {
                flow_run_id: run.id,
                expected_status: FlowRunStatus::Running,
                result: CommitFlowRunTerminalResult::Cancelled {
                    output_payload,
                    error_payload: None,
                },
                flow_run_event_payload: json!({ "reason": "manual_stop" }),
                terminal_event_payload: json!({ "type": "flow_cancelled", "status": "cancelled" }),
                finished_at: OffsetDateTime::now_utc(),
            },
        )
        .await
        .unwrap();
    }

    let messages = <PgControlPlaneStore as ApplicationPublishedFlowRunRepository>::list_assistant_conversation_messages(
        &store,
        seeded.workspace_id,
        seeded.application_id,
        seeded.actor_user_id,
        conversation.conversation_id,
    )
    .await
    .unwrap();
    let assistant_messages = messages
        .iter()
        .filter(|message| message.role == "assistant")
        .collect::<Vec<_>>();
    assert_eq!(assistant_messages.len(), 1);
    assert_eq!(assistant_messages[0].content, "partial answer");
    assert_eq!(assistant_messages[0].status, "cancelled");
}

#[tokio::test]
async fn assistant_conversation_list_filters_user_workspace_application_and_run_source() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;

    let own_conversation = <PgControlPlaneStore as ApplicationPublishedFlowRunRepository>::create_assistant_conversation(
        &store,
        &CreateAssistantConversationInput {
            conversation_id: Uuid::now_v7(),
            workspace_id: seeded.workspace_id,
            application_id: seeded.application_id,
            actor_user_id: seeded.actor_user_id,
            seed_legacy_flow_run_id: None,
        },
    )
    .await
    .unwrap();
    let other_user_id = seed_user(&store, seeded.workspace_id, "assistant-other-user").await;
    let foreign_user_conversation = <PgControlPlaneStore as ApplicationPublishedFlowRunRepository>::create_assistant_conversation(
        &store,
        &CreateAssistantConversationInput {
            conversation_id: Uuid::now_v7(),
            workspace_id: seeded.workspace_id,
            application_id: seeded.application_id,
            actor_user_id: other_user_id,
            seed_legacy_flow_run_id: None,
        },
    )
    .await
    .unwrap();
    let other_application = <PgControlPlaneStore as ApplicationRepository>::create_application(
        &store,
        &CreateApplicationInput {
            actor_user_id: seeded.actor_user_id,
            workspace_id: seeded.workspace_id,
            application_type: ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Other Assistant App".into(),
            description: "other assistant app".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as ApplicationPublishedFlowRunRepository>::create_assistant_conversation(
        &store,
        &CreateAssistantConversationInput {
            conversation_id: Uuid::now_v7(),
            workspace_id: seeded.workspace_id,
            application_id: other_application.id,
            actor_user_id: seeded.actor_user_id,
            seed_legacy_flow_run_id: None,
        },
    )
    .await
    .unwrap();
    let other_scope = seed_runtime_base_with_workspace_name(&store, "Other Assistant Scope").await;
    sqlx::query(
        r#"
        insert into assistant_conversations (
            conversation_id, scope_id, application_id, created_by
        ) values ($1, $2, $3, $4)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(other_scope.workspace_id)
    .bind(other_scope.application_id)
    .bind(seeded.actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();

    let started_at = datetime!(2026-08-06 10:00:00 UTC);
    let legacy_run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "legacy-visible".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::AssistantExecution,
            target_node_id: None,
            title: "Visible legacy run".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({ "query": "visible" }),
            started_at,
            api_key_id: None,
            publication_version_id: None,
            assistant_conversation_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: Some("embedded_assistant".to_string()),
            idempotency_key: None,
        },
    )
    .await
    .unwrap();
    let active_run_input = CreateFlowRunInput {
        actor_user_id: seeded.actor_user_id,
        application_id: seeded.application_id,
        flow_id: seeded.flow_id,
        flow_draft_id: seeded.draft_id,
        compiled_plan_id: compiled.id,
        debug_session_id: "conversation-active".to_string(),
        flow_schema_version: compiled.schema_version.clone(),
        document_hash: compiled.document_hash.clone(),
        run_mode: FlowRunMode::AssistantExecution,
        target_node_id: None,
        title: "Active conversation run".to_string(),
        status: FlowRunStatus::Queued,
        input_payload: json!({ "query": "active" }),
        started_at: started_at + Duration::milliseconds(500),
        api_key_id: None,
        publication_version_id: None,
        assistant_conversation_id: Some(own_conversation.conversation_id),
        external_user: None,
        external_conversation_id: None,
        external_trace_id: None,
        compatibility_mode: Some("embedded_assistant".to_string()),
        idempotency_key: None,
    };
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &active_run_input,
    )
    .await
    .unwrap();
    assert!(
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
            &store,
            &active_run_input,
        )
        .await
        .is_err()
    );
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "wrong-source".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::DebugNodePreview,
            target_node_id: Some("node-start".to_string()),
            title: "Wrong source".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({ "query": "not an assistant run" }),
            started_at: started_at + Duration::seconds(1),
            api_key_id: None,
            publication_version_id: None,
            assistant_conversation_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: Some("embedded_assistant".to_string()),
            idempotency_key: None,
        },
    )
    .await
    .unwrap();

    let page = <PgControlPlaneStore as ApplicationPublishedFlowRunRepository>::list_assistant_conversations(
        &store,
        &ListAssistantConversationsInput {
            workspace_id: seeded.workspace_id,
            application_id: seeded.application_id,
            actor_user_id: seeded.actor_user_id,
            page: 1,
            page_size: 20,
        },
    )
    .await
    .unwrap();
    assert_eq!(page.total, 2);
    assert!(page.items.iter().any(|item| {
        item.conversation_id == Some(own_conversation.conversation_id)
            && item.legacy_flow_run_id.is_none()
            && item.latest_flow_run_status.as_deref() == Some("queued")
    }));
    assert!(page.items.iter().any(|item| {
        item.conversation_id.is_none()
            && item.legacy_flow_run_id == Some(legacy_run.id)
            && item.latest_flow_run_status.as_deref() == Some("running")
    }));
    assert!(
        <PgControlPlaneStore as ApplicationPublishedFlowRunRepository>::has_active_assistant_conversation_run(
            &store,
            own_conversation.conversation_id,
        )
        .await
        .unwrap()
    );

    let foreign_user_access =
        <PgControlPlaneStore as ApplicationPublishedFlowRunRepository>::get_assistant_conversation(
            &store,
            seeded.workspace_id,
            seeded.application_id,
            seeded.actor_user_id,
            foreign_user_conversation.conversation_id,
        )
        .await
        .unwrap();
    assert!(foreign_user_access.is_none());
}
