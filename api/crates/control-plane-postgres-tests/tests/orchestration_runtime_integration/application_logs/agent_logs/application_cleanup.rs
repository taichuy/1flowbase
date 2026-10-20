use super::*;

#[tokio::test]
async fn agent_logs_application_delete_cascades_owned_facts_and_preserves_foreign_scope() {
    let (store, scope, app) = setup().await;
    let owner: Uuid = sqlx::query_scalar("select created_by from applications where id=$1")
        .bind(app)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let foreign_app = store
        .create_application(&CreateApplicationInput {
            actor_user_id: owner,
            workspace_id: scope,
            application_type: ApplicationType::AgentLogs,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Foreign identical source".into(),
            description: "fixture".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    store
        .set_application_api_enabled(&SetApplicationApiEnabledInput {
            actor_user_id: owner,
            application_id: foreign_app.id,
            api_enabled: true,
        })
        .await
        .unwrap();
    let mut completion = event(
        "final",
        4,
        AgentLogEventKind::TaskEnd,
        Some("declared final"),
    );
    completion.phase = Some("final_answer".into());
    let input = batch(vec![
        event(
            "system",
            1,
            AgentLogEventKind::System,
            Some("effective system"),
        ),
        event("user", 2, AgentLogEventKind::User, Some("question")),
        event(
            "tool",
            3,
            AgentLogEventKind::ToolResult,
            Some("observed tool output"),
        ),
        completion.clone(),
    ]);
    let service = AgentLogsService::new(store.clone());
    let id = service
        .ingest(app, scope, Uuid::now_v7(), input.clone())
        .await
        .unwrap()
        .record_ids[0];
    let foreign_id = service
        .ingest(foreign_app.id, scope, Uuid::now_v7(), input)
        .await
        .unwrap()
        .record_ids[0];
    assert_ne!(id, foreign_id);
    let expected = vec![1, 0, 0, 1, 3, 1, 4, 12, 4, 4, 4];
    assert_eq!(owned_log_counts(&store, app, &[id], &[]).await, expected);
    let foreign_before = owned_log_counts(&store, foreign_app.id, &[foreign_id], &[]).await;
    assert_eq!(foreign_before, expected);

    let wrong_scope = seed_workspace(&store, "wrong-delete-workspace").await;
    let wrong = store
        .delete_application(&DeleteApplicationInput {
            actor_user_id: owner,
            workspace_id: wrong_scope,
            application_id: app,
        })
        .await
        .unwrap_err();
    assert!(matches!(
        wrong.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::NotFound("application"))
    ));
    assert_eq!(owned_log_counts(&store, app, &[id], &[]).await, expected);
    assert_eq!(
        owned_log_counts(&store, foreign_app.id, &[foreign_id], &[]).await,
        foreign_before
    );

    let command = DeleteApplicationInput {
        actor_user_id: owner,
        workspace_id: scope,
        application_id: app,
    };
    store.delete_application(&command).await.unwrap();
    assert_eq!(owned_log_counts(&store, app, &[id], &[]).await, vec![0; 11]);
    assert!(store
        .application_log_record(app, id)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        owned_log_counts(&store, foreign_app.id, &[foreign_id], &[]).await,
        foreign_before
    );
    let foreign = store
        .application_log_record(foreign_app.id, foreign_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(foreign.messages.len(), 3);
    assert_eq!(foreign.messages.last().unwrap().content, "declared final");
    let page = store
        .record_client_trajectory_page(foreign_app.id, foreign_id, None, 8)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 4);
    let raw = store
        .record_client_trajectory_section(
            foreign_app.id,
            foreign_id,
            page.items.last().unwrap().id,
            "raw",
            None,
            8,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        raw.items[0].value,
        serde_json::to_value(completion).unwrap()
    );
    let repeated = store.delete_application(&command).await.unwrap_err();
    assert!(matches!(
        repeated.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::NotFound("application"))
    ));
    assert_eq!(
        owned_log_counts(&store, foreign_app.id, &[foreign_id], &[]).await,
        foreign_before
    );
}

#[tokio::test]
async fn agent_logs_application_delete_preserves_native_flow_first_canonical_cleanup() {
    let db = isolated_database().await;
    let store = PgControlPlaneStore::new(db.connect().await.unwrap());
    run_migrations(store.pool()).await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        OffsetDateTime::now_utc(),
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    let request_id = Uuid::now_v7();
    let step_id = Uuid::now_v7();
    let at = "2026-10-07T08:00:00Z";
    let step = ClientTrajectoryStep {
        id: step_id,
        request_id,
        sequence: 1,
        created_at: at.into(),
        category: "assistant".into(),
        name: "Native observed assistant".into(),
        namespace: None,
        preview: "Native final".into(),
        parameters_preview: None,
        result_preview: None,
        status: "observed".into(),
        origin: "native".into(),
        protocol: "responses".into(),
        transport: ClientTrajectoryTransport::Http,
        flow_run_id: Some(run.id),
        node_run_id: None,
        parent_id: None,
        call_id: None,
        item_id: Some("native-final".into()),
        response_id: Some("native-response".into()),
        turn_id: None,
        related_step_id: None,
        available_sections: vec!["result".into()],
    };
    for fact in [
        ClientTrajectoryFact::Integrity {
            status: "pending".into(),
            dropped_count: 0,
            persist_failed_count: 0,
        },
        ClientTrajectoryFact::Step {
            step: Box::new(step),
        },
        ClientTrajectoryFact::Section {
            step_id,
            section: "result".into(),
            value: json!({"content": "Native final"}),
        },
    ] {
        store
            .append_client_trajectory(&AppendClientTrajectoryInput {
                flow_run_id: run.id,
                node_run_id: None,
                request_id,
                observed_at: at.into(),
                fact,
            })
            .await
            .unwrap();
    }
    let before = owned_log_counts(&store, seeded.application_id, &[run.id], &[run.id]).await;
    for index in [0, 1, 2, 3, 5, 6, 7, 9] {
        assert!(
            before[index] > 0,
            "Native ownership precondition {index}: {before:?}"
        );
    }
    store
        .delete_application(&DeleteApplicationInput {
            actor_user_id: seeded.actor_user_id,
            workspace_id: seeded.workspace_id,
            application_id: seeded.application_id,
        })
        .await
        .unwrap();
    assert_eq!(
        owned_log_counts(&store, seeded.application_id, &[run.id], &[run.id]).await,
        vec![0; 11]
    );
}
