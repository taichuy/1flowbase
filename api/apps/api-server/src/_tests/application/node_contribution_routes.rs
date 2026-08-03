use crate::_tests::support::{login_and_capture_cookie, test_app_with_database_url};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn create_application(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    application_type: &str,
    name: &str,
) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": application_type,
                        "name": name,
                        "description": "node contribution test application",
                        "icon": "RobotOutlined",
                        "icon_type": "iconfont",
                        "icon_background": "#E6F7F2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    payload["data"]["id"].as_str().unwrap().to_string()
}

async fn seed_node_contribution_registry(database_url: &str) -> (Uuid, Uuid) {
    let pool = PgPool::connect(database_url).await.unwrap();
    let workspace_id: Uuid =
        sqlx::query_scalar("select id from workspaces order by created_at asc limit 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account = 'root' limit 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let installation_id = Uuid::now_v7();
    let contribution_id = Uuid::now_v7();
    let assignment_id = Uuid::now_v7();

    sqlx::query(
        r#"
        insert into extension_installations (
            id, category, organization, artifact_id, artifact_version, plugin_id,
            contract_version, protocol, display_name, source_kind, trust_level,
            verification_status, desired_state, signature_status, signature_algorithm,
            signing_key_id, metadata_json, created_by
        ) values (
            $1, 'capability-plugins', 'test', 'fixture_provider', '1.2.3',
            'fixture_provider@1.2.3', '1flowbase.capability/v1', 'stdio_json',
            'Fixture Provider', 'uploaded', 'verified_official', 'valid',
            'active_requested', 'verified', 'ed25519', 'fixture-key', '{}', $2
        )
        "#,
    )
    .bind(installation_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into extension_artifact_instances (
            node_id, installation_id, local_version, local_path,
            artifact_status, runtime_status, availability_status
        ) values ($1, $2, '1.2.3', '/tmp/plugins/fixture_provider/1.2.3',
            'ready', 'inactive', 'available')
        "#,
    )
    .bind(crate::_tests::support::test_config().api_node_id)
    .bind(installation_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into plugin_assignments (
            id,
            installation_id,
            workspace_id,
            provider_code,
            assigned_by
        ) values ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(assignment_id)
    .bind(installation_id)
    .bind(workspace_id)
    .bind("fixture_provider")
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into node_contribution_registry (
            id,
            installation_id,
            provider_code,
            plugin_unique_identifier,
            package_id,
            plugin_id,
            plugin_version,
            contribution_code,
            node_shell,
            category,
            title,
            description,
            icon,
            schema_ui,
            schema_version,
            output_schema,
            contribution_checksum,
            compiled_contribution_hash,
            output_schema_snapshot,
            side_effect_policy,
            infra_contracts,
            required_auth,
            visibility,
            experimental,
            dependency_installation_kind,
            dependency_plugin_version_range
        ) values (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18,
            $19, $20, $21, $22, $23, $24, $25, $26
        )
        "#,
    )
    .bind(contribution_id)
    .bind(installation_id)
    .bind("fixture_provider")
    .bind("fixture_provider")
    .bind("fixture_provider@1.2.3")
    .bind("fixture_provider@1.2.3")
    .bind("1.2.3")
    .bind("fixture_prompt")
    .bind("action")
    .bind("ai")
    .bind("Fixture Prompt")
    .bind("Prompt node fixture")
    .bind("spark")
    .bind(json!({"type":"object"}))
    .bind("1flowbase.node-contribution/v2")
    .bind(json!({
        "outputs": [{ "key": "answer", "title": "Answer", "valueType": "string" }]
    }))
    .bind("sha256:contribution")
    .bind("sha256:compiled")
    .bind(json!({
        "outputs": [{ "key": "answer", "title": "Answer", "valueType": "string" }]
    }))
    .bind("external_read")
    .bind(json!([]))
    .bind(json!(["provider_instance"]))
    .bind("public")
    .bind(false)
    .bind("required")
    .bind(">=1.2.3")
    .execute(&pool)
    .await
    .unwrap();

    (workspace_id, actor_id)
}

async fn seed_js_dependency_registry(database_url: &str) -> (Uuid, Uuid, Uuid) {
    let pool = PgPool::connect(database_url).await.unwrap();
    let workspace_id: Uuid =
        sqlx::query_scalar("select id from workspaces order by created_at asc limit 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account = 'root' limit 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let installation_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into extension_installations (
            id, category, organization, artifact_id, artifact_version, plugin_id,
            contract_version, protocol, display_name, source_kind, trust_level,
            verification_status, desired_state, signature_status, metadata_json, created_by
        ) values (
            $1, 'capability-plugins', 'test', 'fixture_js_dependency_pack', '0.1.0',
            'fixture_js_dependency_pack@0.1.0', '1flowbase.capability/v1', 'stdio_json',
            'Fixture JS Dependency Pack', 'uploaded', 'checksum_only', 'valid',
            'active_requested', 'missing', '{}', $2
        )
        "#,
    )
    .bind(installation_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into extension_artifact_instances (
            node_id, installation_id, local_version, local_path,
            artifact_status, runtime_status, availability_status
        ) values ($1, $2, '0.1.0', '/tmp/plugins/fixture_js_dependency_pack/0.1.0',
            'ready', 'inactive', 'available')
        "#,
    )
    .bind(crate::_tests::support::test_config().api_node_id)
    .bind(installation_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into js_dependency_registry (
            id,
            installation_id,
            provider_code,
            plugin_id,
            plugin_version,
            alias,
            package,
            version,
            target,
            artifact_path,
            integrity,
            permission_network,
            permission_filesystem,
            permission_env
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(installation_id)
    .bind("fixture_js_dependency_pack")
    .bind("fixture_js_dependency_pack@0.1.0")
    .bind("0.1.0")
    .bind("zod")
    .bind("zod")
    .bind("3.24.0")
    .bind("backend_code")
    .bind("artifacts/zod.backend.mjs")
    .bind("sha256-zod")
    .bind("outbound_only")
    .bind("deny")
    .bind("deny")
    .execute(&pool)
    .await
    .unwrap();

    (installation_id, workspace_id, actor_id)
}

async fn assign_js_dependency_pack(
    database_url: &str,
    installation_id: Uuid,
    workspace_id: Uuid,
    actor_id: Uuid,
) {
    let pool = PgPool::connect(database_url).await.unwrap();
    sqlx::query(
        r#"
        insert into plugin_assignments (
            id,
            installation_id,
            workspace_id,
            provider_code,
            assigned_by
        ) values ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(installation_id)
    .bind(workspace_id)
    .bind("fixture_js_dependency_pack")
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn node_contribution_route_returns_type_specific_unified_application_node_catalogs() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let agent_flow_id = create_application(
        &app,
        &cookie,
        &csrf,
        "agent_flow",
        "Agent Flow Node Catalog",
    )
    .await;
    let workflow_id =
        create_application(&app, &cookie, &csrf, "workflow", "Workflow Node Catalog").await;
    let _ = seed_node_contribution_registry(&database_url).await;

    let agent_flow_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/node-contributions?application_id={agent_flow_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(agent_flow_response.status(), StatusCode::OK);
    let body = to_bytes(agent_flow_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let agent_flow_payload: Value = serde_json::from_slice(&body).unwrap();
    let agent_flow_nodes = agent_flow_payload["data"]["nodes"].as_array().unwrap();

    // AC-002: boundaries are application-type specific while executable processing nodes and
    // workspace plugin contributions are shared.
    assert!(agent_flow_nodes
        .iter()
        .any(|entry| entry["node_type"] == "start"));
    assert!(agent_flow_nodes
        .iter()
        .any(|entry| entry["node_type"] == "answer"));
    assert!(!agent_flow_nodes
        .iter()
        .any(|entry| entry["node_type"] == "workflow_start"));
    assert!(!agent_flow_nodes
        .iter()
        .any(|entry| entry["node_type"] == "workflow_end"));

    let builtin_categories = ["io", "generation", "control", "data", "external"];
    assert!(agent_flow_nodes
        .iter()
        .filter(|entry| entry["source_kind"] == "builtin")
        .all(|entry| builtin_categories.contains(&entry["category"].as_str().unwrap())));
    for (node_type, category) in [
        ("llm", "generation"),
        ("if_else", "control"),
        ("sql", "data"),
        ("http_request", "external"),
        ("human_input", "io"),
    ] {
        assert_eq!(
            agent_flow_nodes
                .iter()
                .find(|entry| entry["node_type"] == node_type)
                .unwrap()["category"],
            category
        );
    }

    let sql = agent_flow_nodes
        .iter()
        .find(|entry| entry["node_type"] == "sql")
        .expect("SQL must be present in the unified built-in catalog");
    assert_eq!(sql["source_kind"], "builtin");
    assert_eq!(sql["runtime_status"], "ready");
    assert!(sql["field_contract"]["config_fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["key"] == "config.data_source_instance_id"));
    assert!(sql["field_contract"]["input_fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["key"] == "bindings.sql"));

    let llm = agent_flow_nodes
        .iter()
        .find(|entry| entry["node_type"] == "llm")
        .expect("LLM must be present in the unified built-in catalog");
    let llm_config_keys = llm["field_contract"]["config_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field["key"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(llm_config_keys.contains(&"config.model_provider.provider_code"));
    assert!(llm_config_keys.contains(&"config.model_provider.model_id"));
    assert!(!llm_config_keys.contains(&"config.model_provider.source_instance_id"));

    let unavailable = agent_flow_nodes
        .iter()
        .find(|entry| entry["node_type"] == "knowledge_retrieval")
        .expect("known built-in contracts remain discoverable");
    assert_eq!(unavailable["runtime_status"], "unavailable");

    let plugin = agent_flow_nodes
        .iter()
        .find(|entry| entry["source_kind"] == "plugin")
        .expect("assigned workspace plugin contribution must be present");
    assert_eq!(plugin["node_type"], "plugin_node");
    assert_eq!(plugin["runtime_status"], "ready");
    assert_eq!(plugin["dependency_status"], "ready");
    assert_eq!(plugin["plugin"]["plugin_id"], "fixture_provider@1.2.3");
    assert_eq!(
        plugin["plugin"]["plugin_unique_identifier"],
        "fixture_provider"
    );
    assert_eq!(plugin["plugin"]["package_id"], "fixture_provider@1.2.3");
    assert_eq!(plugin["plugin"]["contribution_code"], "fixture_prompt");
    assert_eq!(
        plugin["plugin"]["schema_version"],
        "1flowbase.node-contribution/v2"
    );
    assert_eq!(
        plugin["plugin"]["contribution_checksum"],
        "sha256:contribution"
    );
    assert_eq!(
        plugin["plugin"]["compiled_contribution_hash"],
        "sha256:compiled"
    );

    let workflow_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/node-contributions?application_id={workflow_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(workflow_response.status(), StatusCode::OK);
    let body = to_bytes(workflow_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let workflow_payload: Value = serde_json::from_slice(&body).unwrap();
    let workflow_nodes = workflow_payload["data"]["nodes"].as_array().unwrap();

    assert!(workflow_nodes
        .iter()
        .any(|entry| entry["node_type"] == "workflow_start"));
    assert!(workflow_nodes
        .iter()
        .any(|entry| entry["node_type"] == "workflow_end"));
    assert!(!workflow_nodes
        .iter()
        .any(|entry| entry["node_type"] == "start"));
    assert!(!workflow_nodes
        .iter()
        .any(|entry| entry["node_type"] == "answer"));
    assert!(workflow_nodes
        .iter()
        .any(|entry| entry["node_type"] == "sql"));

    // AC-003: Workflow Start exposes the exact persisted flow-document field vocabulary.
    let workflow_start = workflow_nodes
        .iter()
        .find(|entry| entry["node_type"] == "workflow_start")
        .unwrap();
    let fields = workflow_start["field_contract"]["config_fields"]
        .as_array()
        .unwrap();
    let input_type = fields
        .iter()
        .find(|field| field["key"] == "config.input_fields[].inputType")
        .unwrap();
    assert_eq!(
        input_type["allowed_values"],
        json!([
            "text",
            "paragraph",
            "select",
            "number",
            "checkbox",
            "file",
            "file_list",
            "url"
        ])
    );
    let source = fields
        .iter()
        .find(|field| field["key"] == "config.input_fields[].source")
        .unwrap();
    assert_eq!(
        source["allowed_values"],
        json!(["path", "query", "body", "form"])
    );
    assert!(workflow_nodes.iter().all(|node| {
        node.get("description").is_none() && node.get("runtime_status_description").is_none()
    }));
    assert!(fields.iter().all(|field| {
        field.get("description").is_none() && field.get("applicability").is_none()
    }));
    for property in [
        "key",
        "label",
        "inputType",
        "valueType",
        "required",
        "placeholder",
        "defaultValue",
        "maxLength",
        "hidden",
        "options",
        "source",
    ] {
        assert!(fields
            .iter()
            .any(|field| field["key"] == format!("config.input_fields[].{property}")));
    }
}

#[tokio::test]
async fn js_dependency_route_lists_only_assigned_workspace_catalog() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let (installation_id, workspace_id, actor_id) =
        seed_js_dependency_registry(&database_url).await;
    let hidden_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/js-dependencies")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(hidden_response.status(), StatusCode::OK);
    let hidden_body = to_bytes(hidden_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let hidden_payload: Value = serde_json::from_slice(&hidden_body).unwrap();
    assert!(hidden_payload["data"].as_array().unwrap().is_empty());

    assign_js_dependency_pack(&database_url, installation_id, workspace_id, actor_id).await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/js-dependencies")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    let entry = payload["data"][0].clone();

    assert_eq!(payload["data"].as_array().unwrap().len(), 1);
    assert_eq!(
        entry["provider_code"].as_str(),
        Some("fixture_js_dependency_pack")
    );
    assert_eq!(entry["alias"].as_str(), Some("zod"));
    assert_eq!(entry["package"].as_str(), Some("zod"));
    assert_eq!(entry["version"].as_str(), Some("3.24.0"));
    assert_eq!(entry["target"].as_str(), Some("backend_code"));
    assert_eq!(
        entry["artifact_path"].as_str(),
        Some("artifacts/zod.backend.mjs")
    );
    assert_eq!(entry["integrity"].as_str(), Some("sha256-zod"));
    assert_eq!(
        entry["permissions"]["network"].as_str(),
        Some("outbound_only")
    );
    assert_eq!(entry["permissions"]["filesystem"].as_str(), Some("deny"));
    assert_eq!(entry["permissions"]["env"].as_str(), Some("deny"));
}
