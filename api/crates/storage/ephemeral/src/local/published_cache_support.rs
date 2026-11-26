use std::{future::Future, io, sync::Arc, time::Duration};

use moka::future::Cache;
use uuid::Uuid;

const MAX_WEIGHT_BYTES: u64 = 32 * 1024 * 1024;
const TTL: Duration = Duration::from_secs(5 * 60);

#[derive(Default)]
pub(super) struct ByteCounter(u64);

impl ByteCounter {
    fn finish_weight(self) -> u32 {
        self.0.min(u32::MAX as u64) as u32
    }
}

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

pub(super) fn build_published_cache<V, W>(weigh: W) -> Cache<Uuid, Arc<V>>
where
    V: Send + Sync + 'static,
    W: Fn(&V, &mut ByteCounter) + Send + Sync + 'static,
{
    Cache::builder()
        .max_capacity(MAX_WEIGHT_BYTES)
        .time_to_live(TTL)
        .weigher(move |_, value: &Arc<V>| {
            let mut bytes = ByteCounter::default();
            weigh(value.as_ref(), &mut bytes);
            bytes.finish_weight()
        })
        .build()
}

pub(super) async fn get_or_load_cached<V, F, Fut>(
    cache: &Cache<Uuid, Arc<V>>,
    compiled_plan_id: Uuid,
    loader: F,
) -> anyhow::Result<Option<Arc<V>>>
where
    V: Send + Sync + 'static,
    F: FnOnce() -> Fut,
    Fut: Future<Output = anyhow::Result<Option<V>>>,
{
    match cache
        .try_get_with(compiled_plan_id, async move {
            match loader().await {
                Ok(Some(value)) => Ok(Arc::new(value)),
                Ok(None) => Err(LoadFailure::Missing),
                Err(error) => Err(LoadFailure::Repository(error)),
            }
        })
        .await
    {
        Ok(value) => Ok(Some(value)),
        Err(error) => match error.as_ref() {
            LoadFailure::Missing => Ok(None),
            LoadFailure::Repository(source) => Err(anyhow::anyhow!("{source:#}")),
        },
    }
}
