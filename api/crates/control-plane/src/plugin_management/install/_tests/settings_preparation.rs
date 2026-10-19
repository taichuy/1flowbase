use super::*;
use std::{sync::mpsc, time::Duration};

#[tokio::test(flavor = "current_thread")]
async fn preparation_yields_to_runtime_while_blocking_work_is_pending() {
    let runtime_thread = std::thread::current().id();
    let (started, waiting) = tokio::sync::oneshot::channel();
    let (release, released) = mpsc::channel();
    let task = tokio::spawn(run_preparation(move || {
        assert_ne!(std::thread::current().id(), runtime_thread);
        let _ = started.send(());
        released.recv_timeout(Duration::from_secs(5))?;
        Ok(42)
    }));
    tokio::time::timeout(Duration::from_secs(5), waiting)
        .await
        .unwrap()
        .unwrap();
    tokio::task::yield_now().await;
    assert!(!task.is_finished());
    release.send(()).unwrap();
    assert_eq!(task.await.unwrap().unwrap(), 42);
}

#[tokio::test]
async fn preparation_preserves_validation_errors_and_reports_worker_failure() {
    let error = run_preparation::<()>(|| anyhow::bail!("invalid native settings declaration"))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("invalid native settings declaration"));
    let error = run_preparation::<()>(|| panic!("preparation worker panicked"))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("plugin installation preparation task failed"));
}

fn settings_fixture() -> (std::path::PathBuf, PluginManifestV1) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/fixtures/northwind.settings-page");
    let manifest = plugin_framework::parse_plugin_manifest(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../plugins/fixtures/northwind.settings-page/manifest.yaml"
    )))
    .unwrap();
    (root, manifest)
}

#[tokio::test]
async fn native_settings_preparation_keeps_package_source_and_declaration_validation() {
    let (root, manifest) = settings_fixture();
    let templates = run_preparation(move || prepare_settings_templates(&root, &manifest))
        .await
        .unwrap();
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].feature_id, "northwind.settings-page.settings");
    assert!(!templates[0].source.is_empty());

    let (root, mut manifest) = settings_fixture();
    manifest.runtime.entry = "missing-native-declaration.yaml".to_owned();
    let error = run_preparation(move || prepare_settings_templates(&root, &manifest))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("read native declaration"));
}
