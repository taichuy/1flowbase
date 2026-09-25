use std::sync::Arc;

use uuid::Uuid;

use crate::ports::{ApplicationCompiledPlanRepository, PublishedPlanCache};

pub(crate) async fn load_published_plan<R>(
    repository: &R,
    cache: Option<&Arc<dyn PublishedPlanCache>>,
    compiled_plan_id: Uuid,
) -> anyhow::Result<Option<Arc<domain::CompiledPlanRecord>>>
where
    R: ApplicationCompiledPlanRepository + Clone + Send + Sync + 'static,
{
    if let Some(cache) = cache {
        let repository = repository.clone();
        return cache
            .get_or_load(
                compiled_plan_id,
                Box::new(move || {
                    Box::pin(async move {
                        repository
                            .get_application_compiled_plan(compiled_plan_id)
                            .await
                    })
                }),
            )
            .await;
    }
    repository
        .get_application_compiled_plan(compiled_plan_id)
        .await
        .map(|plan| plan.map(Arc::new))
}
