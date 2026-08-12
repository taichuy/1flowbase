use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use super::*;

async fn create_runtime_block(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    workspace_id: &str,
    page_id: &str,
    tab_id: &str,
    title: &str,
    presentation: &str,
    parent_block_id: Option<&str>,
    code: &str,
) -> String {
    let (status, payload) = send_json(
        app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks"),
        cookie,
        csrf,
        json!({
            "tab_id": tab_id,
            "title": title,
            "presentation": presentation,
            "parent_block_id": parent_block_id,
            "code": code,
            "input_mapping": { "input": format!("{title}.input") },
            "output_mapping": { "output": format!("{title}.output") },
            "runtime_descriptor": { "fixture": title },
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{payload}");
    payload["data"]["block_id"]
        .as_str()
        .expect("created block id")
        .to_owned()
}

fn source_sha256(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}

fn assert_error(payload: &Value, code: &str) {
    assert_eq!(payload["code"], json!(code), "{payload}");
}

#[tokio::test]
async fn runtime_assembly_is_one_visible_root_to_target_public_snapshot() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &root_cookie).await;
    let (_, page_payload) = create_page(
        &app,
        &root_cookie,
        &root_csrf,
        &workspace_id,
        Some("Runtime assembly"),
        None,
        "a",
    )
    .await;
    let page_id = page_payload["data"]["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let tab_id = page_payload["data"]["default_tab"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let root_source = "export default function Root() { return 'root'; }";
    let child_source = "export default function Drawer() { return 'drawer'; }";
    let target_source = "export default function Modal() { return 'modal'; }";
    let inline_source = "export default function Inline() { return 'inline'; }";
    let root_id = create_runtime_block(
        &app,
        &root_cookie,
        &root_csrf,
        &workspace_id,
        &page_id,
        &tab_id,
        "Root",
        "page",
        None,
        root_source,
    )
    .await;
    let child_id = create_runtime_block(
        &app,
        &root_cookie,
        &root_csrf,
        &workspace_id,
        &page_id,
        &tab_id,
        "Drawer",
        "drawer",
        Some(&root_id),
        child_source,
    )
    .await;
    let target_id = create_runtime_block(
        &app,
        &root_cookie,
        &root_csrf,
        &workspace_id,
        &page_id,
        &tab_id,
        "Modal",
        "modal",
        Some(&child_id),
        target_source,
    )
    .await;
    let inline_id = create_runtime_block(
        &app,
        &root_cookie,
        &root_csrf,
        &workspace_id,
        &page_id,
        &tab_id,
        "Inline",
        "inline",
        Some(&root_id),
        inline_source,
    )
    .await;
    let assembly_path = format!(
        "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/{target_id}/runtime-assembly"
    );

    let (status, payload) = get_json(&app, &assembly_path, &root_cookie).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    let layers = payload["data"]["layers"].as_array().unwrap();
    assert_eq!(
        layers.len(),
        3,
        "empty Page Document must not affect Block assembly"
    );
    assert_eq!(
        layers
            .iter()
            .map(|layer| layer["block_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![root_id.as_str(), child_id.as_str(), target_id.as_str()]
    );
    assert_eq!(layers[0]["parent_block_id"], Value::Null);
    assert_eq!(layers[1]["parent_block_id"], json!(root_id));
    assert_eq!(layers[2]["parent_block_id"], json!(child_id));
    assert_eq!(layers[2]["block_id"], json!(target_id));
    assert_eq!(layers[0]["presentation"], json!("page"));
    assert_eq!(layers[1]["presentation"], json!("drawer"));
    assert_eq!(layers[2]["presentation"], json!("modal"));
    assert_eq!(layers[0]["code"], json!(root_source));
    assert_eq!(layers[1]["code"], json!(child_source));
    assert_eq!(layers[2]["code"], json!(target_source));
    assert_eq!(
        layers[0]["source_sha256"],
        json!(source_sha256(root_source))
    );
    assert_eq!(
        layers[1]["source_sha256"],
        json!(source_sha256(child_source))
    );
    assert_eq!(
        layers[2]["source_sha256"],
        json!(source_sha256(target_source))
    );
    assert!(layers.iter().all(|layer| layer["tab_id"] == json!(tab_id)));
    assert_eq!(layers[2]["runtime_descriptor"]["fixture"], json!("Modal"));
    assert_eq!(layers[2]["input_mapping"]["input"], json!("Modal.input"));
    assert_eq!(layers[2]["output_mapping"]["output"], json!("Modal.output"));

    let expected_layer_fields = BTreeSet::from([
        "block_id",
        "tab_id",
        "parent_block_id",
        "title",
        "presentation",
        "schema_version",
        "input_mapping",
        "output_mapping",
        "runtime_descriptor",
        "code",
        "source_sha256",
    ]);
    for layer in layers {
        assert_eq!(
            layer
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            expected_layer_fields
        );
        for leaked in [
            "id",
            "workspace_id",
            "page_id",
            "code_ref",
            "model_code",
            "dataModelCode",
            "physical_table_name",
            "runtime_model_path",
            "blocks",
            "rank",
            "created_at",
            "updated_at",
        ] {
            assert!(layer.get(leaked).is_none(), "leaked {leaked}: {layer}");
        }
    }

    let inline_path = format!(
        "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/{inline_id}/runtime-assembly"
    );
    let (inline_status, inline_payload) = get_json(&app, &inline_path, &root_cookie).await;
    assert_eq!(inline_status, StatusCode::OK, "{inline_payload}");
    assert_eq!(
        inline_payload["data"]["layers"][1]["block_id"],
        json!(inline_id)
    );
    assert_eq!(
        inline_payload["data"]["layers"][1]["presentation"],
        json!("inline")
    );

    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "runtime-assembly-viewer",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "runtime_assembly_viewer").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "runtime_assembly_viewer",
        &[],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["runtime_assembly_viewer"],
    )
    .await;
    let (routes_status, routes_payload) = send_json(
        &app,
        "PUT",
        "/api/console/settings/roles/runtime_assembly_viewer/frontstage-routes",
        &root_cookie,
        &root_csrf,
        json!({ "page_ids": [page_id], "tab_ids": [tab_id] }),
    )
    .await;
    assert_eq!(routes_status, StatusCode::NO_CONTENT, "{routes_payload}");
    let (viewer_cookie, _) =
        login_and_capture_cookie(&app, "runtime-assembly-viewer", "temp-pass").await;
    let (visible_status, visible_payload) = get_json(&app, &assembly_path, &viewer_cookie).await;
    assert_eq!(visible_status, StatusCode::OK, "{visible_payload}");

    let (hide_status, hide_payload) = send_json(
        &app,
        "PUT",
        "/api/console/settings/roles/runtime_assembly_viewer/frontstage-routes",
        &root_cookie,
        &root_csrf,
        json!({ "page_ids": [], "tab_ids": [] }),
    )
    .await;
    assert_eq!(hide_status, StatusCode::NO_CONTENT, "{hide_payload}");
    let (hidden_status, hidden_payload) = get_json(&app, &assembly_path, &viewer_cookie).await;
    assert_eq!(hidden_status, StatusCode::NOT_FOUND, "{hidden_payload}");
    assert_error(&hidden_payload, "block_node_not_found");

    let (_, other_page_payload) = create_page(
        &app,
        &root_cookie,
        &root_csrf,
        &workspace_id,
        Some("Other page"),
        None,
        "b",
    )
    .await;
    let other_page_id = other_page_payload["data"]["page"]["id"].as_str().unwrap();
    let (cross_page_status, cross_page_payload) = get_json(
        &app,
        &format!(
            "/api/console/frontstage/{workspace_id}/pages/{other_page_id}/blocks/{target_id}/runtime-assembly"
        ),
        &root_cookie,
    )
    .await;
    assert_eq!(
        cross_page_status,
        StatusCode::NOT_FOUND,
        "{cross_page_payload}"
    );
    assert_error(&cross_page_payload, "block_node_not_found");

    let wrong_workspace_id = uuid::Uuid::now_v7();
    let (wrong_workspace_status, _) = get_json(
        &app,
        &format!(
            "/api/console/frontstage/{wrong_workspace_id}/pages/{page_id}/blocks/{target_id}/runtime-assembly"
        ),
        &root_cookie,
    )
    .await;
    assert_eq!(wrong_workspace_status, StatusCode::FORBIDDEN);

    let (missing_status, missing_payload) = get_json(
        &app,
        &format!(
            "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/missing/runtime-assembly"
        ),
        &root_cookie,
    )
    .await;
    assert_eq!(missing_status, StatusCode::NOT_FOUND, "{missing_payload}");
    assert_error(&missing_payload, "block_node_not_found");

    let (delete_status, delete_payload) = send_json(
        &app,
        "DELETE",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/{target_id}"),
        &root_cookie,
        &root_csrf,
        json!({}),
    )
    .await;
    assert_eq!(delete_status, StatusCode::NO_CONTENT, "{delete_payload}");
    let (deleted_status, deleted_payload) = get_json(&app, &assembly_path, &root_cookie).await;
    assert_eq!(deleted_status, StatusCode::NOT_FOUND, "{deleted_payload}");
    assert_error(&deleted_payload, "block_node_not_found");

    let openapi = serde_json::to_value(crate::openapi::ApiDoc::openapi()).unwrap();
    let openapi_path = format!(
        "/api/console/frontstage/{{workspace_id}}/pages/{{page_id}}/blocks/{{block_id}}/runtime-assembly"
    );
    assert_eq!(
        openapi["paths"][&openapi_path]["get"]["operationId"],
        json!("get_frontstage_block_runtime_assembly")
    );
    let operation = crate::openapi_docs::DocsCatalogOperation {
        id: "get_frontstage_block_runtime_assembly".into(),
        method: "GET".into(),
        path: openapi_path.clone(),
        summary: None,
        description: None,
        tags: Vec::new(),
        group: "other".into(),
        deprecated: false,
    };
    let interface = crate::openapi_interface::catalog_entry_from_operation(&operation, &openapi)
        .expect("runtime assembly interface catalog entry");
    let response_validator = jsonschema::validator_for(&interface.response_schema).unwrap();
    assert!(response_validator.validate(&payload["data"]).is_ok());

    let (catalog_status, catalog_payload) = get_json(
        &app,
        "/api/console/mcp/interface-capabilities",
        &root_cookie,
    )
    .await;
    assert_eq!(catalog_status, StatusCode::OK, "{catalog_payload}");
    let catalog_entry = catalog_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["interface_id"] == "get_frontstage_block_runtime_assembly")
        .expect("runtime assembly must be in compiled interface inventory");
    assert_eq!(catalog_entry["method"], json!("GET"));
    assert_eq!(catalog_entry["path"], json!(openapi_path));
    assert_eq!(catalog_entry["bindable"], json!(true));
}
