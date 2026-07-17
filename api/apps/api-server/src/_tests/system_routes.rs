use crate::_tests::support::{
    get_json, sample_api_profile, sample_runner_profile, test_app_with_runtime_profile_error,
    test_app_with_runtime_profiles,
};

#[tokio::test]
async fn runtime_profile_merges_same_host_services() {
    let (app, cookie) = test_app_with_runtime_profiles(
        sample_api_profile("host_same"),
        Some(sample_runner_profile("host_same")),
        &["settings_feature.access.system.system-runtime"],
        Some("zh_Hans"),
    )
    .await;

    let payload = get_json(&app, "/api/console/system/runtime-profile", &cookie).await;
    assert_eq!(payload["data"]["topology"]["relationship"], "same_host");
    assert_eq!(payload["data"]["hosts"].as_array().unwrap().len(), 1);
    let runtime_targets = payload["data"]["runtime_targets"].as_array().unwrap();
    assert_eq!(runtime_targets.len(), 2);
    assert_eq!(runtime_targets[0]["target_id"], "api-server");
    assert_eq!(runtime_targets[1]["target_id"], "plugin-runner");
    assert_eq!(
        runtime_targets[0]["metrics"]["cpu"]["availability"],
        "available"
    );
    assert_eq!(
        runtime_targets[0]["metrics"]["network"]["received_bytes_per_second"],
        2048.0
    );
    assert_eq!(
        payload["data"]["locale_meta"]["source"],
        "user_preferred_locale"
    );
    let provider_install_root = payload["data"]["provider_install_root"].as_str().unwrap();
    let host_extension_dropin_root = payload["data"]["host_extension_dropin_root"]
        .as_str()
        .unwrap();
    assert!(provider_install_root.contains("api-provider-plugins-"));
    assert_eq!(
        host_extension_dropin_root,
        format!("{provider_install_root}/host-extension/dropins")
    );
}

#[tokio::test]
async fn runtime_profile_reports_runner_unreachable_without_failing_request() {
    let (app, cookie) =
        test_app_with_runtime_profile_error(&["settings_feature.access.system.system-runtime"])
            .await;

    let payload = get_json(&app, "/api/console/system/runtime-profile", &cookie).await;
    assert_eq!(
        payload["data"]["topology"]["relationship"],
        "runner_unreachable"
    );
    assert_eq!(
        payload["data"]["services"]["plugin_runner"]["reachable"],
        false
    );
}
