use api_server::host_extension_boot::builtin_host_extension_ids;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[test]
fn builtin_host_extensions_include_plan_f_official_hosts() {
    let ids = builtin_host_extension_ids();

    assert_eq!(
        ids,
        vec![
            "official.identity-host",
            "official.workspace-host",
            "official.plugin-host",
            "official.local-infra-host",
            "official.file-management-host",
            "official.runtime-orchestration-host",
        ]
    );
}

pub(crate) use crate::native_settings_fixture::NativeSettingsFixture;
use control_plane::ports::{PluginRepository, UiManagementRepository};
use domain::{PluginDesiredState, PluginRuntimeStatus};

#[tokio::test]
async fn root_2014_ac_014_template_commit_failure_keeps_target_pending() {
    let f = NativeSettingsFixture::new().await;
    let target = f.select("1.0.0").await;
    // Two pages exercise rollback after an earlier revision was already inserted.
    sqlx::query("insert into plugin_settings_template_defaults(installation_id,contribution_code,feature_id,source,language) values($1,'zz','northwind.settings-page.zz','reject','tsx')").bind(target.installation_id).execute(f.state.store.pool()).await.unwrap();
    sqlx::query("alter table ui_code_template_revisions add constraint controlled_apply_failure check(source <> 'reject')").execute(f.state.store.pool()).await.unwrap();
    let prepared = f.prepare().await;
    assert!(prepared
        .apply_templates(&f.state.store, &f.state.api_node_id)
        .await
        .is_err());
    assert!(f
        .state
        .store
        .list_ui_code_templates()
        .await
        .unwrap()
        .is_empty());
    assert!(!f
        .state
        .store
        .native_plugin_target_is_applied(&target)
        .await
        .unwrap());
    assert_eq!(
        f.state
            .store
            .get_installation(target.installation_id)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        PluginDesiredState::PendingRestart
    );
    assert_eq!(
        f.state
            .store
            .get_artifact_instance(&f.state.api_node_id, target.installation_id)
            .await
            .unwrap()
            .unwrap()
            .runtime_status,
        PluginRuntimeStatus::LoadFailed
    );
    let artifact = f
        .state
        .store
        .get_artifact_instance(&f.state.api_node_id, target.installation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        artifact.artifact_status,
        domain::PluginArtifactInstanceStatus::LoadFailed
    );
    assert_eq!(
        artifact.availability_status,
        domain::PluginAvailabilityStatus::LoadFailed
    );
    // Listing a failed target must not make readiness-based installation lookups succeed.
    let by_id =
        control_plane::ports::ExtensionInstallationRepository::find_extension_installation_by_id(
            &f.state.store,
            &f.state.api_node_id,
            target.installation_id,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(by_id.status, domain::ExtensionInstallationStatus::Missing);
    let by_identity =
        control_plane::ports::ExtensionInstallationRepository::find_extension_installation(
            &f.state.store,
            &f.state.api_node_id,
            &by_id.identity,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        by_identity.status,
        domain::ExtensionInstallationStatus::Missing
    );
    let app = crate::app_with_state(f.state.clone());
    let (cookie, _) =
        crate::_tests::support::login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .oneshot(Request::builder()
            .uri("/api/console/settings/extension-center/installed?category=host-extensions&limit=50")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let selected = body["data"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == target.installation_id.to_string())
        .expect("installed list must retain the precise failed native target");
    assert_eq!(selected["desired_state"], "pending_restart");
    assert_eq!(selected["runtime_status"], "load_failed");
    assert_eq!(selected["availability_status"], "load_failed");
    sqlx::query("alter table ui_code_template_revisions drop constraint controlled_apply_failure")
        .execute(f.state.store.pool())
        .await
        .unwrap();
    let prepared = f.prepare().await;
    prepared
        .apply_templates(&f.state.store, &f.state.api_node_id)
        .await
        .unwrap();
    crate::host_extension_loader::activate_prepared_host_extensions(
        &f.state.store,
        &f.state.api_node_id,
        prepared,
    )
    .await
    .unwrap();
    assert!(f
        .state
        .store
        .native_plugin_target_is_applied(&target)
        .await
        .unwrap());
}

#[tokio::test]
async fn root_2014_ac_014_post_apply_boot_failure_retry_preserves_user_edit() {
    let f = NativeSettingsFixture::new().await;
    let target = f.select("1.0.0").await;
    let prepared = f.prepare().await;
    prepared
        .apply_templates(&f.state.store, &f.state.api_node_id)
        .await
        .unwrap();
    crate::host_extension_loader::fail_after_template_apply_for(target.installation_id);
    assert!(
        crate::host_extension_loader::activate_prepared_host_extensions(
            &f.state.store,
            &f.state.api_node_id,
            prepared
        )
        .await
        .is_err()
    );
    assert!(f
        .state
        .store
        .native_plugin_target_is_applied(&target)
        .await
        .unwrap());
    assert_eq!(
        f.state
            .store
            .get_artifact_instance(&f.state.api_node_id, target.installation_id)
            .await
            .unwrap()
            .unwrap()
            .runtime_status,
        PluginRuntimeStatus::LoadFailed
    );
    let id = f
        .edit("export default () => <p>after committed application</p>")
        .await;
    let prepared = f.prepare().await;
    prepared
        .apply_templates(&f.state.store, &f.state.api_node_id)
        .await
        .unwrap();
    crate::host_extension_loader::activate_prepared_host_extensions(
        &f.state.store,
        &f.state.api_node_id,
        prepared,
    )
    .await
    .unwrap();
    assert_eq!(
        f.state
            .store
            .get_ui_code_template(id)
            .await
            .unwrap()
            .unwrap()
            .latest_revision
            .source,
        "export default () => <p>after committed application</p>"
    );
    assert_eq!(
        f.state
            .store
            .get_artifact_instance(&f.state.api_node_id, target.installation_id)
            .await
            .unwrap()
            .unwrap()
            .runtime_status,
        PluginRuntimeStatus::Active
    );
}
