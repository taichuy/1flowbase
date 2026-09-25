use std::{io, sync::Arc, time::Duration};

use async_trait::async_trait;
use control_plane_contracts::{
    application_public_api::ApplicationPublicationVersionRecord,
    ports::{PublishedPublicationCache, PublishedPublicationLoader},
};
use moka::future::Cache;
use uuid::Uuid;

const MAX_WEIGHT_BYTES: u64 = 32 * 1024 * 1024;
const TTL: Duration = Duration::from_secs(5 * 60);

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
            publications: Cache::builder()
                .max_capacity(MAX_WEIGHT_BYTES)
                .time_to_live(TTL)
                .weigher(
                    |_, publication: &Arc<ApplicationPublicationVersionRecord>| {
                        let mut bytes = ByteCounter::default();
                        let _ = serde_json::to_writer(&mut bytes, &publication.document_snapshot);
                        let _ = serde_json::to_writer(&mut bytes, &publication.mapping_snapshot);
                        let _ = serde_json::to_writer(
                            &mut bytes,
                            &publication.runtime_profile_snapshot,
                        );
                        bytes.0.min(u32::MAX as u64) as u32
                    },
                )
                .build(),
        }
    }
}

#[derive(Default)]
struct ByteCounter(u64);

impl io::Write for ByteCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self.0.saturating_add(bytes.len() as u64);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
enum LoadFailure {
    Missing,
    Repository(anyhow::Error),
}

#[async_trait]
impl PublishedPublicationCache for MokaPublishedPublicationCache {
    async fn get_or_load(
        &self,
        compiled_plan_id: Uuid,
        loader: PublishedPublicationLoader,
    ) -> anyhow::Result<Option<Arc<ApplicationPublicationVersionRecord>>> {
        match self
            .publications
            .try_get_with(compiled_plan_id, async move {
                match loader().await {
                    Ok(Some(publication)) => Ok(Arc::new(publication)),
                    Ok(None) => Err(LoadFailure::Missing),
                    Err(error) => Err(LoadFailure::Repository(error)),
                }
            })
            .await
        {
            Ok(publication) => Ok(Some(publication)),
            Err(error) => match error.as_ref() {
                LoadFailure::Missing => Ok(None),
                LoadFailure::Repository(source) => Err(anyhow::anyhow!("{source:#}")),
            },
        }
    }
}
