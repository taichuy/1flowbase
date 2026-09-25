use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use uuid::Uuid;

use super::*;
use crate::{
    application_public_api::{
        mapping::ApplicationApiMappingConfig,
        publications::{
            ApplicationPublicationService, PublishApplicationCommand,
            SetApplicationApiEnabledCommand, UnpublishApplicationCommand,
        },
        ApplicationPublicApiTestHarness,
    },
    ports::{PublishedPublicationCache, PublishedPublicationLoader},
};

#[derive(Default)]
struct TestPublicationCache(Mutex<HashMap<Uuid, Arc<ApplicationPublicationVersionRecord>>>);

#[async_trait]
impl PublishedPublicationCache for TestPublicationCache {
    async fn get_or_load(
        &self,
        compiled_plan_id: Uuid,
        loader: PublishedPublicationLoader,
    ) -> anyhow::Result<Option<Arc<ApplicationPublicationVersionRecord>>> {
        if let Some(snapshot) = self.0.lock().unwrap().get(&compiled_plan_id) {
            return Ok(Some(snapshot.clone()));
        }
        let snapshot = loader().await?.map(Arc::new);
        if let Some(snapshot) = &snapshot {
            self.0
                .lock()
                .unwrap()
                .insert(compiled_plan_id, snapshot.clone());
        }
        Ok(snapshot)
    }
}

#[tokio::test]
async fn republish_and_disable_take_effect_with_a_warm_snapshot_cache() {
    let harness = ApplicationPublicApiTestHarness::new();
    let actor_user_id = Uuid::from_u128(0x11111111111111111111111111111111);
    let application = harness.seed_application(actor_user_id, "Published cache lifecycle");
    let repository = harness.repository();
    let service = ApplicationPublicationService::new(repository.clone());
    let cache: Arc<dyn PublishedPublicationCache> = Arc::new(TestPublicationCache::default());
    let publish = || PublishApplicationCommand {
        actor_user_id,
        application_id: application.id,
        mapping: ApplicationApiMappingConfig::default_native(),
        api_enabled: true,
    };

    let first = service.publish_active_version(publish()).await.unwrap();
    let warm = load_active_publication(&repository, Some(&cache), application.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(warm.snapshot.compiled_plan_id, first.compiled_plan_id);
    assert!(warm.api_enabled);

    service
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id,
            application_id: application.id,
            api_enabled: false,
        })
        .await
        .unwrap();
    let disabled = load_active_publication(&repository, Some(&cache), application.id)
        .await
        .unwrap()
        .unwrap();
    assert!(!disabled.api_enabled);

    let second = service.publish_active_version(publish()).await.unwrap();
    assert_eq!(second.id, first.id);
    assert_ne!(second.compiled_plan_id, first.compiled_plan_id);
    let republished = load_active_publication(&repository, Some(&cache), application.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        republished.snapshot.compiled_plan_id,
        second.compiled_plan_id
    );
    assert!(republished.api_enabled);

    service
        .unpublish(UnpublishApplicationCommand {
            actor_user_id,
            application_id: application.id,
        })
        .await
        .unwrap();
    assert!(
        load_active_publication(&repository, Some(&cache), application.id)
            .await
            .unwrap()
            .is_none()
    );
}
