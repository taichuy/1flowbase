//! A new system service must coexist with the unchanged older workspace hook snapshot.
use super::managed_create_pair::Fixture;
use crate::{
    _tests::support::test_config, managed_publication::ManagedGenerationPublisher,
    managed_services::ManagedServiceRegistration,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

#[tokio::test]
async fn hot_generation_new_service_freezes_against_older_workspace_snapshot() {
    let fixture = Fixture::new().await;
    let composition = fixture
        .state
        .provider_runtime
        .managed_composition()
        .unwrap();
    let workspace_id = fixture.actor.current_workspace_id;
    let previous = composition.snapshot(workspace_id).await.unwrap();
    assert!(
        !previous.bindings.is_empty(),
        "fixture must retain actual workspace workers"
    );
    let interface_id = "coexistence.items.list";
    for phase in plugin_framework::extension_bus::MANAGED_INTERFACE_PHASES {
        let point_id = extension_contracts::managed_interface_hook_point_id(interface_id, phase);
        assert!(!previous
            .graph
            .points()
            .iter()
            .any(|p| p.descriptor().point_id.as_str() == point_id));
        assert!(!previous.graph.contribution_receipts().iter().any(|r| r
            .descriptor()
            .point_id
            .as_str()
            == point_id));
    }
    let registration = ManagedServiceRegistration {
        installation_id: uuid::Uuid::now_v7(),
        plugin_code: "coexistence".into(),
        plugin_version: "1.0.0".into(),
        declaration: serde_json::from_value(json!({
            "scope":"system",
            "feature":{"feature_id":"coexistence.settings","label":"Coexistence service","description":"Configure coexistence service","route_id":"coexistence","path":"/settings/coexistence"},
            "operations":[{
                "interface_id":interface_id,"contribution_id":"coexistence.items.list",
                "method":"GET","path":"/api/console/managed-services/coexistence/items",
                "summary":"List coexistence items","description":"List coexistence fixture items.",
                "input_schema":{"type":"object","properties":{"path":{"type":"object"},"query":{"type":"object"},"body":{"type":"object"}}},
                "output_schema":{"type":"object"}
            }]
        })).unwrap(),
        pages: vec![plugin_framework::PluginSettingsPageManifest {
            feature_id: "coexistence.settings".into(), contribution_code: "settings".into(),
            source_file: "ui/Settings.tsx".into(), language: "tsx".into(),
        }],
    };
    let publisher =
        ManagedGenerationPublisher::new(&fixture.state, test_config(), vec![], fixture.app.clone())
            .unwrap();
    let ingress = publisher.ingress();
    let snapshots = composition.current_snapshots().await;
    assert!(Arc::ptr_eq(
        snapshots.get(&workspace_id).unwrap(),
        &previous
    ));
    assert!(!snapshots.contains_key(&domain::SYSTEM_SCOPE_ID));
    publisher.publish(
        publisher
            .prepare(vec![registration], snapshots)
            .await
            .unwrap(),
    );

    let response = ingress
        .oneshot(
            Request::builder()
                .uri("/api/console/managed-services/coexistence/items")
                .header("cookie", &fixture.cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    // The absent system execution is deliberate: only the real typed handler emits this code.
    // A freeze rejection (including managed-point-not-compiled) cannot satisfy this assertion.
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "managed_service_inactive", "{body}");
    assert!(Arc::ptr_eq(
        &composition.snapshot(workspace_id).await.unwrap(),
        &previous
    ));
}
