use std::sync::Arc;

use async_trait::async_trait;
use control_plane_contracts::ports::{PublishedPlanCache, PublishedPlanLoader};
use domain::CompiledPlanRecord;
use moka::future::Cache;
use uuid::Uuid;

use super::published_cache_support::{build_published_cache, get_or_load_cached};

#[derive(Clone)]
pub struct MokaPublishedPlanCache {
    plans: Cache<Uuid, Arc<CompiledPlanRecord>>,
}

impl Default for MokaPublishedPlanCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MokaPublishedPlanCache {
    pub fn new() -> Self {
        Self {
            plans: build_published_cache(|plan: &CompiledPlanRecord, bytes| {
                let _ = serde_json::to_writer(bytes, &plan.plan);
            }),
        }
    }
}

#[async_trait]
impl PublishedPlanCache for MokaPublishedPlanCache {
    async fn get_or_load(
        &self,
        compiled_plan_id: Uuid,
        loader: PublishedPlanLoader,
    ) -> anyhow::Result<Option<Arc<CompiledPlanRecord>>> {
        get_or_load_cached(&self.plans, compiled_plan_id, loader).await
    }
}
