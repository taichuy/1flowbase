use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    i18n_catalog::RuntimeI18nCatalogService,
    ports::{RuntimeCatalogMessage, RuntimeCatalogProjection, RuntimeI18nCatalogRepository},
};
use domain::{CatalogLocale, WorkspaceCatalogRevision};

#[derive(Clone)]
struct ProjectionRepository(Arc<Mutex<RuntimeCatalogProjection>>);

#[async_trait]
impl RuntimeI18nCatalogRepository for ProjectionRepository {
    async fn project_runtime_catalog(
        &self,
        _workspace_id: Uuid,
        _locale: &CatalogLocale,
    ) -> anyhow::Result<RuntimeCatalogProjection> {
        Ok(self.0.lock().unwrap().clone())
    }
}

fn message(key: &str, value: &str) -> RuntimeCatalogMessage {
    RuntimeCatalogMessage {
        key: key.to_owned(),
        value: value.to_owned(),
        raw_key_fallback: key == value,
    }
}

#[tokio::test]
async fn digest_and_canonical_body_are_stable_for_the_global_catalog() {
    let root_workspace_id = Uuid::now_v7();
    let projection = Arc::new(Mutex::new(RuntimeCatalogProjection {
        revision: WorkspaceCatalogRevision::new(3).unwrap(),
        messages: vec![message("Save", "保存"), message("Cancel", "取消")],
    }));
    let service =
        RuntimeI18nCatalogService::new(ProjectionRepository(projection.clone()), root_workspace_id);
    let locale = CatalogLocale::new("zh_Hans").unwrap();

    for forbidden_scope in [domain::SYSTEM_SCOPE_ID, Uuid::now_v7()] {
        assert!(service.manifest(forbidden_scope, &locale).await.is_err());
    }

    let first = service.manifest(root_workspace_id, &locale).await.unwrap();
    let stable = service.manifest(root_workspace_id, &locale).await.unwrap();
    assert_eq!(first, stable);
    assert_eq!(
        first.bundle.canonical_body().unwrap(),
        stable.bundle.canonical_body().unwrap()
    );

    projection.lock().unwrap().messages[0].value = "储存".to_owned();
    projection.lock().unwrap().revision = WorkspaceCatalogRevision::new(4).unwrap();
    let changed = service.manifest(root_workspace_id, &locale).await.unwrap();
    assert_ne!(first.digest, changed.digest);
    assert_ne!(first.bundle, changed.bundle);
}

#[tokio::test]
async fn bundle_is_sorted_resolved_content_without_revision_or_timestamp() {
    let root_workspace_id = Uuid::now_v7();
    let service = RuntimeI18nCatalogService::new(
        ProjectionRepository(Arc::new(Mutex::new(RuntimeCatalogProjection {
            revision: WorkspaceCatalogRevision::new(99).unwrap(),
            messages: vec![
                message("Zulu", "override"),
                message("Alpha", "official"),
                message("Custom key", "custom"),
                message("Fallback", "Fallback"),
            ],
        }))),
        root_workspace_id,
    );
    let manifest = service
        .manifest(root_workspace_id, &CatalogLocale::new("zh_Hans").unwrap())
        .await
        .unwrap();
    let body = String::from_utf8(manifest.bundle.canonical_body().unwrap()).unwrap();
    assert_eq!(body, "{\"locale\":\"zh_Hans\",\"messages\":{\"Alpha\":\"official\",\"Custom key\":\"custom\",\"Fallback\":\"Fallback\",\"Zulu\":\"override\"}}");
    assert!(!body.contains("revision"));
    assert!(!body.contains("generated_at"));
}
