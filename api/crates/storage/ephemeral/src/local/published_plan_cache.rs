use std::{io, sync::Arc, time::Duration};

use async_trait::async_trait;
use control_plane_contracts::ports::{PublishedPlanCache, PublishedPlanLoader};
use domain::CompiledPlanRecord;
use moka::future::Cache;
use uuid::Uuid;

const MAX_WEIGHT_BYTES: u64 = 32 * 1024 * 1024;
const TTL: Duration = Duration::from_secs(5 * 60);

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
            plans: Cache::builder()
                .max_capacity(MAX_WEIGHT_BYTES)
                .time_to_live(TTL)
                .weigher(|_, plan: &Arc<CompiledPlanRecord>| {
                    let mut bytes = ByteCounter::default();
                    let _ = serde_json::to_writer(&mut bytes, &plan.plan);
                    bytes.0.min(u32::MAX as u64) as u32
                })
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
impl PublishedPlanCache for MokaPublishedPlanCache {
    async fn get_or_load(
        &self,
        compiled_plan_id: Uuid,
        loader: PublishedPlanLoader,
    ) -> anyhow::Result<Option<Arc<CompiledPlanRecord>>> {
        match self
            .plans
            .try_get_with(compiled_plan_id, async move {
                match loader().await {
                    Ok(Some(plan)) => Ok(Arc::new(plan)),
                    Ok(None) => Err(LoadFailure::Missing),
                    Err(error) => Err(LoadFailure::Repository(error)),
                }
            })
            .await
        {
            Ok(plan) => Ok(Some(plan)),
            Err(error) => match error.as_ref() {
                LoadFailure::Missing => Ok(None),
                LoadFailure::Repository(source) => Err(anyhow::anyhow!("{source:#}")),
            },
        }
    }
}
