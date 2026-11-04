use crate::_tests::support::test_api_state_with_database_url;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

struct DropProbe {
    root: PathBuf,
    seen: Arc<AtomicU8>,
}
impl Drop for DropProbe {
    fn drop(&mut self) {
        self.seen
            .store(if self.root.exists() { 1 } else { 2 }, Ordering::SeqCst);
    }
}
#[async_trait::async_trait]
impl runtime_extension_host::RuntimeArtifactResolver for DropProbe {
    async fn resolve(
        &self,
        artifact: &runtime_core::runtime_backend::RuntimeArtifactReference,
    ) -> Result<PathBuf, runtime_core::runtime_backend::RuntimeBackendError> {
        Ok(PathBuf::from(artifact.as_str()))
    }
}

#[tokio::test]
async fn fixture_resources_outlive_state_clones_and_release_after_last_provider_owner() {
    let (state, _) = test_api_state_with_database_url().await;
    let root = PathBuf::from(&state.provider_install_root);
    std::fs::create_dir_all(&root).unwrap();
    let owner = state.provider_runtime.fixture_resource_owner();
    let resources = owner.upgrade().unwrap();
    let graph = Arc::clone(state.extension_boot_snapshot.as_ref().unwrap().graph_arc());
    let seen = Arc::new(AtomicU8::new(0));
    let backend = Arc::new(
        runtime_extension_host::RuntimeExtensionHost::new_with_artifact_resolver(
            time::OffsetDateTime::now_utc(),
            Arc::new(DropProbe {
                root: root.clone(),
                seen: Arc::clone(&seen),
            }),
        )
        .unwrap(),
    );
    let runtime = Arc::new(
        crate::provider_runtime::ApiRuntimeServices::new_with_runtime_backend(backend, graph)
            .unwrap()
            .with_fixture_resources(resources),
    );
    let state_clone = Arc::clone(&state);
    let provider = crate::provider_runtime::ApiProviderRuntime::new(Arc::clone(&runtime));
    drop(state);
    assert!(root.exists());
    assert!(owner.upgrade().is_some());
    drop(state_clone);
    drop(runtime);
    assert!(root.exists());
    assert!(owner.upgrade().is_some());
    assert_eq!(seen.load(Ordering::SeqCst), 0);
    drop(provider);
    assert_eq!(
        seen.load(Ordering::SeqCst),
        1,
        "the real runtime backend must drop while its fixture files still exist"
    );
    assert!(owner.upgrade().is_none());
    assert!(!root.exists());
}
