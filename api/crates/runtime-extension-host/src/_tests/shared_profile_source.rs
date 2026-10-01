use super::*;

#[test]
fn host_and_api_profiles_use_the_injected_same_process_observation() {
    let source = Arc::new(RuntimeSampleSource::new(std::time::Duration::from_secs(1)));
    let started = OffsetDateTime::now_utc();
    let host = RuntimeExtensionHost::new_with_artifact_resolver_plugin_data_and_profile_source(
        started,
        Arc::new(MissingRuntimeArtifactResolver),
        Arc::new(MissingPluginDataPort),
        source.clone(),
    )
    .unwrap();
    let api = RuntimeProfileCollector::new_with_sample_source(
        "api-server",
        "test",
        started,
        "ok",
        source,
    )
    .unwrap();
    let api_profile = api.collect().unwrap();
    let host_profile = host.collect_runtime_profile().unwrap();
    assert_eq!(api_profile.metrics, host_profile.metrics);
    assert_eq!(api_profile.started_at, host_profile.started_at);
    assert_eq!(api_profile.service, "api-server");
    assert_eq!(host_profile.service, "runtime-extension-host");
}
