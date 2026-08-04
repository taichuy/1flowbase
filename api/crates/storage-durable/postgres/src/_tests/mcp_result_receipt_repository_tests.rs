use control_plane::ports::{
    McpOperationOutcome, McpResultReceiptRepository, RecordMcpResultReceiptInput,
    MCP_RESULT_RECEIPT_SUMMARY_MAX_BYTES,
};
use storage_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn receipt_store() -> (
    PgControlPlaneStore,
    domain::WorkspaceRecord,
    domain::WorkspaceRecord,
    domain::UserRecord,
) {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "MCP Receipt Workspace")
        .await
        .unwrap();
    let other_workspace = store
        .upsert_workspace(tenant.id, "Other MCP Receipt Workspace")
        .await
        .unwrap();
    store
        .upsert_permission_catalog(&access_control::permission_catalog())
        .await
        .unwrap();
    store.upsert_builtin_roles(workspace.id).await.unwrap();
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            public_ui_block: String::new(),
            options: serde_json::json!({}),
        })
        .await
        .unwrap();
    let actor = store
        .upsert_root_user(
            workspace.id,
            "receipt-owner",
            "receipt-owner@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Receipt",
            "Owner",
        )
        .await
        .unwrap();

    (store, workspace, other_workspace, actor)
}

#[tokio::test]
async fn root_1569_ac_007_ac_008_ac_009_receipt_is_scoped_stable_compact_and_durable() {
    let (store, workspace, other_workspace, actor) = receipt_store().await;
    let receipt_id = Uuid::now_v7();
    let input = RecordMcpResultReceiptInput {
        receipt_id,
        workspace_id: workspace.id,
        actor_user_id: actor.id,
        operation_id: "import_mcp_bundle_library_release".into(),
        outcome: McpOperationOutcome::Succeeded,
        summary: serde_json::json!({
            "changes": 7,
            "already_present": 14,
            "failed": 0,
            "bundle_id": "1flowbase_zh_hans"
        }),
    };

    let first = store.record_mcp_result_receipt(&input).await.unwrap();
    assert_eq!(first.receipt_id, receipt_id);
    assert_eq!(first.workspace_id, workspace.id);
    assert_eq!(first.actor_user_id, Some(actor.id));
    assert_eq!(first.operation_id, input.operation_id);
    assert_eq!(first.outcome, McpOperationOutcome::Succeeded);
    assert_eq!(first.summary, input.summary);

    let stored: (Uuid, String, serde_json::Value) =
        sqlx::query_as("select target_id, event_code, payload from audit_logs where id = $1")
            .bind(receipt_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(stored.0, receipt_id);
    assert_eq!(stored.1, "mcp.operation.completed");
    assert_eq!(stored.2["summary"], input.summary);
    assert!(stored.2.get("result").is_none());

    let replay = store
        .record_mcp_result_receipt(&RecordMcpResultReceiptInput {
            summary: serde_json::json!({ "changes": 999 }),
            outcome: McpOperationOutcome::Failed,
            ..input.clone()
        })
        .await
        .unwrap();
    assert_eq!(replay, first, "a stable receipt ID must remain immutable");

    assert!(store
        .get_mcp_result_receipt(other_workspace.id, receipt_id)
        .await
        .unwrap()
        .is_none());
    assert!(store
        .get_mcp_result_receipt(workspace.id, Uuid::now_v7())
        .await
        .unwrap()
        .is_none());

    sqlx::query("update audit_logs set created_at = now() - interval '10 years' where id = $1")
        .bind(receipt_id)
        .execute(store.pool())
        .await
        .unwrap();
    let aged = store
        .get_mcp_result_receipt(workspace.id, receipt_id)
        .await
        .unwrap()
        .expect("durable receipts do not expire with ephemeral detail");
    assert_eq!(aged.receipt_id, receipt_id);

    let audit_count: i64 = sqlx::query_scalar("select count(*) from audit_logs where id = $1")
        .bind(receipt_id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(audit_count, 1);
}

#[tokio::test]
async fn root_1569_ac_007_receipt_rejects_large_detail_instead_of_persisting_it() {
    let (store, workspace, _other_workspace, actor) = receipt_store().await;
    let receipt_id = Uuid::now_v7();
    let error = store
        .record_mcp_result_receipt(&RecordMcpResultReceiptInput {
            receipt_id,
            workspace_id: workspace.id,
            actor_user_id: actor.id,
            operation_id: "import_mcp_bundle_library_release".into(),
            outcome: McpOperationOutcome::Succeeded,
            summary: serde_json::json!({
                "detail": "x".repeat(MCP_RESULT_RECEIPT_SUMMARY_MAX_BYTES + 1)
            }),
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("summary exceeds"));
    assert!(store
        .get_mcp_result_receipt(workspace.id, receipt_id)
        .await
        .unwrap()
        .is_none());
}
