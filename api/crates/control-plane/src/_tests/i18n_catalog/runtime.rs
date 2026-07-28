use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    i18n_catalog::RuntimeI18nCatalogService,
    ports::{RuntimeCatalogMessage, RuntimeCatalogProjection, RuntimeI18nCatalogRepository},
};
use domain::{CatalogLocale, CatalogModuleId, WorkspaceCatalogRevision};

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

fn message(module: &str, msgid: &str, value: &str) -> RuntimeCatalogMessage {
    RuntimeCatalogMessage {
        module: CatalogModuleId::new(module).unwrap(),
        msgid: msgid.to_owned(),
        value: value.to_owned(),
    }
}

#[tokio::test]
async fn ac_011_digest_and_canonical_body_are_stable_and_module_local() {
    let root_workspace_id = Uuid::now_v7();
    let projection = Arc::new(Mutex::new(RuntimeCatalogProjection {
        revision: WorkspaceCatalogRevision::new(3).unwrap(),
        messages: vec![
            message("@taichuy/platform/a", "Save", "保存"),
            message("@taichuy/platform/b", "Cancel", "取消"),
        ],
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
        first.modules[0].bundle.canonical_body().unwrap(),
        stable.modules[0].bundle.canonical_body().unwrap()
    );

    projection.lock().unwrap().messages[0].value = "储存".to_owned();
    projection.lock().unwrap().revision = WorkspaceCatalogRevision::new(4).unwrap();
    let changed = service.manifest(root_workspace_id, &locale).await.unwrap();
    assert_ne!(first.modules[0].digest, changed.modules[0].digest);
    assert_eq!(first.modules[1].digest, changed.modules[1].digest);
    assert_eq!(first.modules[1].bundle, changed.modules[1].bundle);
}

#[tokio::test]
async fn ac_011_bundle_is_sorted_resolved_content_without_revision_or_timestamp() {
    let root_workspace_id = Uuid::now_v7();
    let service = RuntimeI18nCatalogService::new(
        ProjectionRepository(Arc::new(Mutex::new(RuntimeCatalogProjection {
            revision: WorkspaceCatalogRevision::new(99).unwrap(),
            messages: vec![
                message("@taichuy/platform/common", "Zulu", "override"),
                message("@taichuy/platform/common", "Alpha", "official"),
                message("@taichuy/platform/common", "custom.key", "custom"),
                message("@taichuy/platform/common", "Fallback", "Fallback"),
            ],
        }))),
        root_workspace_id,
    );
    let module = CatalogModuleId::new("@taichuy/platform/common").unwrap();
    let bundle = service
        .current_bundle(
            root_workspace_id,
            &module,
            &CatalogLocale::new("zh_Hans").unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    let body = String::from_utf8(bundle.bundle.canonical_body().unwrap()).unwrap();
    assert_eq!(body, "{\"module\":\"@taichuy/platform/common\",\"locale\":\"zh_Hans\",\"messages\":{\"Alpha\":\"official\",\"Fallback\":\"Fallback\",\"Zulu\":\"override\",\"custom.key\":\"custom\"}}");
    assert!(!body.contains("revision"));
    assert!(!body.contains("generated_at"));
}
