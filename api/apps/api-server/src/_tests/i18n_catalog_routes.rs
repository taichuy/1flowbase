use utoipa::OpenApi;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use control_plane::{
    plugin_management::{
        ExtensionArtifactInstallOutcome, ExtensionCatalogCategory, ExtensionInstallationService,
        InstallExtensionArtifactCommand,
    },
    ports::{AuthRepository, I18nCatalogRepository},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

use crate::_tests::support::{
    login_and_capture_cookie, test_api_state_with_database_url, test_config,
};

#[test]
fn settings_i18n_routes_expose_state_and_management_without_module_bundle_contract() {
    let assembly = crate::routes::console_route_assembly::migrated_core_console_route_assembly();
    assert!(assembly.bindings().iter().any(|binding| {
        binding.route.method == "GET" && binding.route.path == "/api/console/settings/i18n/catalog"
    }));
    assert!(!assembly
        .bindings()
        .iter()
        .any(|binding| binding.route.path.contains("/i18n/modules/")));

    let openapi = serde_json::to_value(crate::openapi::ApiDoc::openapi()).unwrap();
    assert!(openapi["paths"]
        .as_object()
        .unwrap()
        .keys()
        .all(|path| !path.contains("/i18n/modules/")));
    let state_schema = &openapi["components"]["schemas"]["I18nCatalogStateResponse"];
    let properties = state_schema["properties"].as_object().unwrap();
    assert!(properties.get("modules").is_none());
    assert!(properties.get("module").is_none());
    assert!(properties.get("msgid").is_none());
}

#[tokio::test]
async fn delivery_1545_d6_installed_i18n_catalog_previews_and_activates_local_artifact() {
    let (state, _) = test_api_state_with_database_url().await;
    I18nCatalogRepository::bootstrap_workspace_catalog_state(
        &state.store,
        state.bootstrap_workspace_id,
    )
    .await
    .unwrap();
    let actor = AuthRepository::find_user_for_password_login(
        &state.store,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        "root",
    )
    .await
    .unwrap()
    .unwrap();
    let bytes = crate::official_i18n_catalog_seed::OFFICIAL_SEED_BYTES.to_vec();
    let outcome =
        ExtensionInstallationService::new(state.store.clone(), &state.provider_install_root)
            .install_from_bytes(InstallExtensionArtifactCommand {
                actor_user_id: actor.id,
                category: ExtensionCatalogCategory::I18n,
                organization: "taichuy".into(),
                artifact_id: "platform".into(),
                version: "2.0.4".into(),
                node_id: state.api_node_id.clone(),
                artifact_bytes: bytes.clone(),
                source: "official".into(),
                trust: "official".into(),
                expected_checksum: Some(format!("sha256:{:x}", Sha256::digest(&bytes))),
                signature_status: domain::ExtensionSignatureStatus::Verified,
                signature_algorithm: Some("ed25519".into()),
                signing_key_id: Some("official-key".into()),
                declared_warnings: Vec::new(),
                risk_override: None,
                confirmation_receipt: None,
                application_action: domain::ExtensionApplicationAction::ActivateI18n,
            })
            .await
            .unwrap();
    let ExtensionArtifactInstallOutcome::Installed { installation, .. } = outcome else {
        panic!("fixture must install");
    };
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let preview_path = format!(
        "/api/console/settings/i18n/installed-extension/{}/preview",
        installation.id
    );
    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&preview_path)
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview.status(), StatusCode::OK);
    let preview: Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    let revision = preview["data"]["revision"].as_i64().unwrap();
    assert_eq!(preview["data"]["installed_catalog_version"], "2.0.4");

    let activate = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/i18n/installed-extension/{}/activate",
                    installation.id
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "expected_revision": revision }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(activate.status(), StatusCode::OK);
}
