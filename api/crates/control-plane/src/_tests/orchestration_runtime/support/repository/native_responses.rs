use super::*;

impl InMemoryOrchestrationRuntimeRepository {
    pub(crate) async fn enable_native_responses_fixture(&self) {
        let (installation_id, manifest_path) = {
            let inner = self.inner.lock().unwrap();
            let instance = inner
                .instances_by_id
                .get(&self.default_provider_instance_id)
                .expect("default fixture provider");
            let artifact = inner
                .artifact_instances_by_key
                .values()
                .find(|artifact| artifact.installation_id == instance.installation_id)
                .unwrap();
            (
                instance.installation_id,
                std::path::Path::new(artifact.local_path.as_ref().unwrap()).join("manifest.yaml"),
            )
        };
        let raw = std::fs::read_to_string(&manifest_path).unwrap();
        let marker = "  entry: bin/fixture_provider-provider\n";
        assert_eq!(
            raw.matches(marker).count(),
            1,
            "fixture runtime entry must be unique"
        );
        let native = raw.replace(marker,
            "  entry: bin/fixture_provider-provider\n  capabilities:\n    - responses.native_passthrough\n    - responses.native_output.v1\n    - native_continuation_supported\n");
        std::fs::write(&manifest_path, native).unwrap();
        let fingerprint = plugin_framework::compute_manifest_fingerprint(&manifest_path)
            .await
            .unwrap();
        let mut inner = self.inner.lock().unwrap();
        inner
            .artifact_instances_by_key
            .values_mut()
            .find(|artifact| artifact.installation_id == installation_id)
            .unwrap()
            .manifest_fingerprint = Some(fingerprint);
    }
}
