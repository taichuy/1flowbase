use std::sync::Arc;

use uuid::Uuid;

use super::OrchestrationRuntimeService;
use crate::ports::{OrchestrationRuntimeRepository, PublishedPlanCache, PublishedPlanLoader};

impl<R, H> OrchestrationRuntimeService<R, H> {
    pub fn with_published_plan_cache(mut self, cache: Arc<dyn PublishedPlanCache>) -> Self {
        self.published_plan_cache = Some(cache);
        self
    }
}

impl<R, H> OrchestrationRuntimeService<R, H>
where
    R: OrchestrationRuntimeRepository + Clone + Send + Sync + 'static,
{
    // The caller resolves and authorizes the run before loading its immutable ID.
    pub(super) async fn load_frozen_compiled_plan(
        &self,
        compiled_plan_id: Uuid,
    ) -> anyhow::Result<Option<Arc<domain::CompiledPlanRecord>>> {
        let repository = self.repository.clone();
        load_frozen_plan(
            self.published_plan_cache.as_ref(),
            compiled_plan_id,
            Box::new(move || {
                Box::pin(async move { repository.get_compiled_plan(compiled_plan_id).await })
            }),
        )
        .await
    }
}

async fn load_frozen_plan(
    cache: Option<&Arc<dyn PublishedPlanCache>>,
    compiled_plan_id: Uuid,
    loader: PublishedPlanLoader,
) -> anyhow::Result<Option<Arc<domain::CompiledPlanRecord>>> {
    match cache {
        Some(cache) => cache.get_or_load(compiled_plan_id, loader).await,
        None => loader().await.map(|plan| plan.map(Arc::new)),
    }
}

#[cfg(test)]
mod _tests;
