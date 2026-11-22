use std::sync::Arc;

use async_trait::async_trait;
use control_plane_contracts::{
    application_public_api::ApplicationPublicationVersionRecord,
    ports::{PublishedPublicationCache, PublishedPublicationLoader},
};
use moka::future::Cache;
use uuid::Uuid;

use super::published_cache_support::{build_published_cache, get_or_load_cached};

#[derive(Clone)]
pub struct MokaPublishedPublicationCache {
    publications: Cache<Uuid, Arc<ApplicationPublicationVersionRecord>>,
}

impl Default for MokaPublishedPublicationCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MokaPublishedPublicationCache {
    pub fn new() -> Self {
        Self {
            publications: build_published_cache(
                |publication: &ApplicationPublicationVersionRecord, bytes| {
                    let _ = serde_json::to_writer(&mut *bytes, &publication.document_snapshot);
                    let _ = serde_json::to_writer(&mut *bytes, &publication.mapping_snapshot);
                    let _ =
                        serde_json::to_writer(&mut *bytes, &publication.runtime_profile_snapshot);
                },
            ),
        }
    }
}

#[async_trait]
impl PublishedPublicationCache for MokaPublishedPublicationCache {
    async fn get_or_load(
        &self,
        compiled_plan_id: Uuid,
        loader: PublishedPublicationLoader,
    ) -> anyhow::Result<Option<Arc<ApplicationPublicationVersionRecord>>> {
        get_or_load_cached(&self.publications, compiled_plan_id, loader).await
    }
}
