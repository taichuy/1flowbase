use std::sync::Arc;

use uuid::Uuid;

use super::publications::ApplicationPublicationVersionRecord;
use crate::ports::{ApplicationPublicationRepository, PublishedPublicationCache};

#[cfg(test)]
#[path = "../_tests/application_public_api/published_publication_cache.rs"]
mod tests;

pub(crate) struct ActivePublishedPublication {
    pub snapshot: Arc<ApplicationPublicationVersionRecord>,
    pub api_enabled: bool,
}

pub(crate) async fn load_active_publication<R>(
    repository: &R,
    cache: Option<&Arc<dyn PublishedPublicationCache>>,
    application_id: Uuid,
) -> anyhow::Result<Option<ActivePublishedPublication>>
where
    R: ApplicationPublicationRepository + Clone + Send + Sync + 'static,
{
    let Some(cache) = cache else {
        return repository
            .load_active_application_publication(application_id)
            .await
            .map(|publication| {
                publication.map(|snapshot| ActivePublishedPublication {
                    api_enabled: snapshot.api_enabled,
                    snapshot: Arc::new(snapshot),
                })
            });
    };
    for _ in 0..2 {
        let Some(active) = repository
            .load_active_application_publication_identity(application_id)
            .await?
        else {
            return Ok(None);
        };
        let repository = repository.clone();
        let snapshot = cache
            .get_or_load(
                active.compiled_plan_id,
                Box::new(move || {
                    Box::pin(async move {
                        let snapshot = repository
                            .get_application_publication_version(active.publication_id)
                            .await?;
                        Ok(snapshot.filter(|snapshot| {
                            snapshot.compiled_plan_id == active.compiled_plan_id
                        }))
                    })
                }),
            )
            .await?;
        if let Some(snapshot) = snapshot {
            return Ok(Some(ActivePublishedPublication {
                snapshot,
                api_enabled: active.api_enabled,
            }));
        }
    }
    anyhow::bail!("active publication changed while loading its snapshot")
}
