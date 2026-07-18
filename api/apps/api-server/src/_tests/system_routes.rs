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
    assert_eq!(payload["data"]["related_process_memory_complete"], true);
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
        runtime_targets[0]["metrics"]["memory"]["process_bytes"],
        256 * 1024 * 1024
    );
    assert_eq!(
        runtime_targets[0]["metrics"]["memory"]["related_process_bytes"],
        320 * 1024 * 1024
    );
    assert_eq!(
        runtime_targets[0]["metrics"]["memory"]["related_process_count"],
        2
    );
    assert_eq!(
        payload["data"]["hosts"][0]["related_process_bytes"],
        768 * 1024 * 1024
    );
    assert_eq!(payload["data"]["hosts"][0]["related_process_count"], 5);
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
async fn ac_008_runtime_profile_keeps_related_process_totals_separate_across_hosts() {
    let (app, cookie) = test_app_with_runtime_profiles(
        sample_api_profile("host_api"),
        Some(sample_runner_profile("host_runner")),
        &["settings_feature.access.system.system-runtime"],
        None,
    )
    .await;

    let payload = get_json(&app, "/api/console/system/runtime-profile", &cookie).await;
    let hosts = payload["data"]["hosts"]
        .as_array()
        .expect("split-host response should contain host groups");

    assert_eq!(hosts.len(), 2);
    assert_eq!(hosts[0]["host_fingerprint"], "host_api");
    assert_eq!(hosts[0]["related_process_bytes"], 320 * 1024 * 1024);
    assert_eq!(hosts[0]["related_process_count"], 2);
    assert_eq!(hosts[1]["host_fingerprint"], "host_runner");
    assert_eq!(hosts[1]["related_process_bytes"], 448 * 1024 * 1024);
    assert_eq!(hosts[1]["related_process_count"], 3);
}

#[tokio::test]
async fn ac_010_runtime_profile_exposes_available_cgroup_memory_composition() {
    let mut api_profile = sample_api_profile("host_cgroup");
    api_profile.metrics.memory.scope_kind = runtime_profile::RuntimeMetricScopeKind::Cgroup;
    api_profile.metrics.memory.cgroup_composition =
        Some(runtime_profile::RuntimeCgroupMemoryComposition {
            anonymous_bytes: Some(512 * 1024 * 1024),
            file_bytes: Some(256 * 1024 * 1024),
            kernel_bytes: Some(64 * 1024 * 1024),
            shared_memory_bytes: None,
        });
    let (app, cookie) = test_app_with_runtime_profiles(
        api_profile,
        None,
        &["settings_feature.access.system.system-runtime"],
        None,
    )
    .await;

    let payload = get_json(&app, "/api/console/system/runtime-profile", &cookie).await;
    let composition =
        &payload["data"]["runtime_targets"][0]["metrics"]["memory"]["cgroup_composition"];

    assert_eq!(composition["anonymous_bytes"], 512 * 1024 * 1024);
    assert_eq!(composition["file_bytes"], 256 * 1024 * 1024);
    assert_eq!(composition["kernel_bytes"], 64 * 1024 * 1024);
    assert_eq!(composition["shared_memory_bytes"], serde_json::Value::Null);
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
    assert_eq!(payload["data"]["related_process_memory_complete"], false);
}

#[tokio::test]
async fn ac_006_runtime_profile_cache_keeps_locale_resolution_request_scoped() {
    let (app, cookie) = test_app_with_runtime_profiles(
        sample_api_profile("host_same"),
        Some(sample_runner_profile("host_same")),
        &["settings_feature.access.system.system-runtime"],
        None,
    )
    .await;

    let english = get_json(
        &app,
        "/api/console/system/runtime-profile?locale=en_US",
        &cookie,
    )
    .await;
    let chinese = get_json(
        &app,
        "/api/console/system/runtime-profile?locale=zh_Hans",
        &cookie,
    )
    .await;

    assert_eq!(english["data"]["locale_meta"]["resolved_locale"], "en_US");
    assert_eq!(chinese["data"]["locale_meta"]["resolved_locale"], "zh_Hans");
    assert_eq!(
        english["data"]["runtime_targets"],
        chinese["data"]["runtime_targets"]
    );
}
