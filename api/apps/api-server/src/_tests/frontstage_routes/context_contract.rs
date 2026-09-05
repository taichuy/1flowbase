use super::*;

#[tokio::test]
async fn ac_001_002_block_context_contract_is_authenticated_and_complete() {
    let app = test_app().await;

    let anonymous = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/frontstage/block-context-contract")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    let (root_cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/frontstage/block-context-contract")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let contract = &payload["data"];
    assert_eq!(
        contract["schema_version"],
        json!("1flowbase.block-context-contract/v1")
    );
    assert_eq!(contract["contract_version"], json!("1.0.0"));
    assert_eq!(contract["block_sdk_version"], json!("1.0.0"));
    let entries = contract["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 17);
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry["key"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "root",
            "assets",
            "currentUser",
            "workspace",
            "application",
            "page",
            "inputs",
            "outputs",
            "params",
            "props",
            "state",
            "patch",
            "api",
            "events",
            "navigation",
            "theme",
            "ui",
        ]
    );
    assert!(entries.iter().all(|entry| {
        entry["description"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
            && entry["type"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
            && entry["members"].is_array()
    }));
    assert_eq!(
        contract["non_context_symbols"],
        json!([
            "React",
            "antd",
            "router",
            "store",
            "queryClient",
            "window",
            "document",
            "fetch",
            "storage"
        ])
    );

    let interface_response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/mcp/interface-capabilities?bindable_only=true")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(interface_response.status(), StatusCode::OK);
    let interface_payload: Value = serde_json::from_slice(
        &to_bytes(interface_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let interface = interface_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["path"] == json!("/api/console/frontstage/block-context-contract"))
        .expect("BlockContext contract interface must be discoverable");
    assert_eq!(
        interface["interface_id"],
        json!("get_frontstage_block_context_contract")
    );
    assert_eq!(interface["method"], json!("GET"));
    assert_eq!(interface["risk_level"], json!("low"));
    assert_eq!(interface["bindable"], json!(true));
}

#[test]
fn ac_001_embedded_contract_rejects_invalid_versions_and_duplicate_names() {
    let source = include_bytes!("../../../resources/ctx/block-context.v1.json");
    let mut invalid_version: Value = serde_json::from_slice(source).unwrap();
    invalid_version["contract_version"] = json!("2.0.0");
    assert!(
        crate::routes::frontstage::block_context_contract::decode_block_context_contract(
            &serde_json::to_vec(&invalid_version).unwrap()
        )
        .is_err()
    );

    let mut duplicate: Value = serde_json::from_slice(source).unwrap();
    duplicate["entries"][1]["key"] = json!("root");
    assert!(
        crate::routes::frontstage::block_context_contract::decode_block_context_contract(
            &serde_json::to_vec(&duplicate).unwrap()
        )
        .is_err()
    );
}

#[test]
fn ac_004_backend_contract_keys_match_the_frontend_protocol_source() {
    let protocol_source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../web/packages/page-protocol/src/block-context.ts"
    ));
    let keys_source = protocol_source
        .split_once("export const BLOCK_CONTEXT_KEYS = [")
        .unwrap()
        .1
        .split_once("] as const;")
        .unwrap()
        .0;
    let frontend_keys = keys_source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_end_matches(',').trim_matches('\''))
        .collect::<Vec<_>>();

    let contract: Value = serde_json::from_slice(include_bytes!(
        "../../../resources/ctx/block-context.v1.json"
    ))
    .unwrap();
    let backend_keys = contract["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["key"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(backend_keys, frontend_keys);

    let block_sdk_source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../web/packages/block-sdk/src/index.ts"
    ));
    let block_sdk_version = block_sdk_source
        .split_once("export const blockSdkVersion = '")
        .unwrap()
        .1
        .split_once("';")
        .unwrap()
        .0;
    assert_eq!(contract["block_sdk_version"], json!(block_sdk_version));
}
