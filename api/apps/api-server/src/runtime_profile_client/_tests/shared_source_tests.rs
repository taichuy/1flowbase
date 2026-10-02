use super::super::{ApiRuntimeProfilePort, HostApiRuntimeProfileCollector};
use runtime_profile::{RuntimeProcessSampler, RuntimeSampleSource};
use std::{sync::Arc, time::Duration};

#[tokio::test]
async fn api_profile_port_and_process_list_share_the_injected_observation() {
    let source = Arc::new(RuntimeSampleSource::new(Duration::from_secs(1)));
    let api = HostApiRuntimeProfileCollector::new_with_sample_source(
        time::OffsetDateTime::now_utc(),
        source.clone(),
    )
    .unwrap();
    let profile = api.collect_runtime_profile().await.unwrap();
    let processes = RuntimeProcessSampler::with_sample_source(source.clone()).collect();
    let shared = source.collect().unwrap();
    assert!(processes
        .processes
        .iter()
        .any(|process| process.pid == std::process::id()));
    assert_eq!(profile.metrics, shared.metrics);
    assert_eq!(
        processes,
        RuntimeProcessSampler::with_sample_source(source).collect()
    );
    assert_eq!(profile.service, "api-server");
}
