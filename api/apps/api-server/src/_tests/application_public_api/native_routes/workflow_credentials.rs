use super::super::workflow_credentials_fixture::save_document;
use super::*;
use control_plane_contracts::ports::WorkflowRunIdentityRepository;

#[tokio::test]
async fn workflow_credentials_native_publication_without_mcp_uses_creator_host_and_denies_nonuser_executor(
) {
    let (app, state) = test_app_with_state().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    for subject in ["creator", "executor"] {
        let application_id = create_application(
            &app,
            &cookie,
            &csrf,
            &format!("Native {subject} credential"),
        )
        .await;
        save_document(&app, &cookie, &csrf, &application_id, false, subject).await;
        let mut mapping = mapping_with_runnable_generate_target();
        mapping["output"]["answer_selector"] = json!("node-http.body");
        publish_native_application(&app, &cookie, &csrf, &application_id, mapping).await;
        let token = create_application_key(&app, &cookie, &csrf, &application_id).await;
        let response = post_native_run(
            &app,
            &token,
            json!({"query":"current identity","response_mode":"blocking"}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let payload = response_json(response).await;
        let run_id: Uuid = payload["data"]["id"].as_str().unwrap().parse().unwrap();
        let snapshot = state
            .store
            .get_workflow_run_identity(run_id)
            .await
            .unwrap()
            .expect("formal Native runs must freeze the host identities even without MCP");
        assert!(
            snapshot.executor.is_none(),
            "application keys never become user executor identities"
        );
        if subject == "creator" {
            assert_eq!(payload["data"]["status"], "succeeded", "{payload}");
            let identity: Value =
                serde_json::from_str(payload["data"]["answer"].as_str().unwrap()).unwrap();
            assert_eq!(identity["data"]["account"], "root");
            assert_eq!(identity["data"]["effective_display_role"], "root");
            assert_eq!(
                identity["data"]["id"],
                snapshot.creator.as_ref().unwrap().user_id.to_string()
            );
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/console/me")
                        .header("authorization", format!("Bearer wfcred:{run_id}:creator"))
                        .header("x-workflow-run-id", run_id.to_string())
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        } else {
            assert_eq!(payload["data"]["status"], "failed", "{payload}");
            let detail = state
                .store
                .get_flow_run(application_id.parse().unwrap(), run_id)
                .await
                .unwrap()
                .unwrap();
            assert!(serde_json::to_string(&detail.error_payload)
                .unwrap()
                .contains("workflow_executor_credential_unavailable"));
        }
    }
}
