use super::*;
use crate::_tests::NativeSettingsFixture;
use axum::{http::StatusCode, response::IntoResponse};
use control_plane_contracts::ports::PluginRepository;

fn asset_input(version: &str, sha256: &str) -> UiManagementInput {
    UiManagementInput::PluginSettingsModuleAsset {
        route_id: "northwind.settings-page.settings".into(),
        applied_plugin_version: version.into(),
        sha256: sha256.into(),
    }
}
fn adapter(state: &crate::app_state::ApiState) -> UiManagementAdapter {
    UiManagementAdapter::for_test(UiManagementDependencies {
        store: state.store.clone(),
        api_node_id: state.api_node_id.clone(),
        surfaces: state.console_surface_registry.clone(),
        network_egress: state.network_egress_http_clients(),
    })
}

#[tokio::test]
async fn settings_reader_assets_are_version_bound_without_frontstage_permissions_or_assignments() {
    let fixture = NativeSettingsFixture::new().await;
    let first = fixture.select_with_modules("1.0.0", true).await;
    fixture
        .state
        .store
        .apply_native_plugin_settings_templates(&first)
        .await
        .unwrap();
    let state = fixture.process_state("1.0.0", first);
    let allowed_actor = domain::ActorContext::scoped(
        fixture.actor,
        uuid::Uuid::nil(),
        "member",
        ["settings_feature.access.northwind.settings-page.settings".to_string()],
    );
    assert!(
        access_control::ensure_permission(&allowed_actor, "frontstage.components.view").is_err()
    );
    let allowed = UserPrincipal::server_delegation(allowed_actor);
    let denied = UserPrincipal::server_delegation(domain::ActorContext::scoped(
        fixture.actor,
        uuid::Uuid::nil(),
        "member",
        [],
    ));
    let module = &state
        .console_surface_registry
        .page("northwind.settings-page.settings")
        .unwrap()
        .code_modules[0];
    let digest = &module.assets[0].sha256;
    let UiManagementOutput::PluginSettingsModuleAsset(asset) = adapter(&state)
        .execute_inner(&allowed, asset_input("1.0.0", digest))
        .await
        .unwrap()
    else {
        panic!("asset response expected")
    };
    assert_eq!(asset.bytes, b"export const version = '1.0.0';");
    assert_eq!(asset.sha256, *digest);
    for (principal, input, expected) in [
        (&denied, asset_input("1.0.0", digest), StatusCode::FORBIDDEN),
        (&allowed, asset_input("2.0.0", digest), StatusCode::CONFLICT),
        (
            &allowed,
            asset_input("1.0.0", &"a".repeat(64)),
            StatusCode::NOT_FOUND,
        ),
    ] {
        let error = adapter(&state)
            .execute_inner(principal, input)
            .await
            .err()
            .unwrap();
        assert_eq!(error.into_response().status(), expected);
    }
    let second = fixture.select_with_modules("2.0.0", true).await;
    fixture
        .state
        .store
        .apply_native_plugin_settings_templates(&second)
        .await
        .unwrap();
    let error = adapter(&state)
        .execute_inner(&allowed, asset_input("1.0.0", digest))
        .await
        .err()
        .unwrap();
    assert_eq!(error.into_response().status(), StatusCode::CONFLICT);
    let new_state = fixture.process_state("2.0.0", second);
    let digest = &new_state
        .console_surface_registry
        .page("northwind.settings-page.settings")
        .unwrap()
        .code_modules[0]
        .assets[0]
        .sha256;
    let UiManagementOutput::PluginSettingsModuleAsset(asset) = adapter(&new_state)
        .execute_inner(&allowed, asset_input("2.0.0", digest))
        .await
        .unwrap()
    else {
        panic!("asset response expected")
    };
    assert_eq!(asset.bytes, b"export const version = '2.0.0';");
    // Stored bytes are independently verified before they can leave the settings asset API.
    sqlx::query("update retained_frontend_module_assets set bytes = $1 where sha256 = $2")
        .bind(b"tampered".to_vec())
        .bind(digest)
        .execute(new_state.store.pool())
        .await
        .unwrap();
    let error = adapter(&new_state)
        .execute_inner(&allowed, asset_input("2.0.0", digest))
        .await
        .err()
        .unwrap();
    assert_eq!(error.into_response().status(), StatusCode::CONFLICT);
}
