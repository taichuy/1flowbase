//! One on-demand OS observation shared by same-process profile projections.
use crate::{
    RuntimeMetricSampler, RuntimeMetricsSnapshot, RuntimeProcessSnapshot,
    RuntimeProcessTerminationOutcome,
};
use anyhow::{anyhow, Result};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct RuntimeSampleSnapshot {
    pub metrics: RuntimeMetricsSnapshot,
    pub processes: Arc<RuntimeProcessSnapshot>,
}
#[derive(Debug)]
struct SampleSourceInner {
    sampler: RuntimeMetricSampler,
    completed_at: Option<Instant>,
    snapshot: Option<Arc<RuntimeSampleSnapshot>>,
    #[cfg(test)]
    raw_refresh_count: usize,
}
/// Composition Root owns this source; it has no timer or persistent writes.
/// The mutex combines concurrent misses and snapshots remain immutable after publication.
#[derive(Debug)]
pub struct RuntimeSampleSource {
    freshness: Duration,
    inner: Mutex<SampleSourceInner>,
}
impl RuntimeSampleSource {
    pub fn new(freshness: Duration) -> Self {
        Self {
            freshness: freshness.max(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL),
            inner: Mutex::new(SampleSourceInner {
                sampler: RuntimeMetricSampler::new(),
                completed_at: None,
                snapshot: None,
                #[cfg(test)]
                raw_refresh_count: 0,
            }),
        }
    }
    pub fn collect(&self) -> Result<Arc<RuntimeSampleSnapshot>> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow!("runtime sample source lock poisoned"))?;
        if inner
            .completed_at
            .is_some_and(|at| at.elapsed() < self.freshness)
        {
            if let Some(snapshot) = &inner.snapshot {
                return Ok(Arc::clone(snapshot));
            }
        }
        let metrics = inner.sampler.collect();
        let processes = Arc::new(inner.sampler.process_snapshot());
        #[cfg(test)]
        {
            inner.raw_refresh_count += 1;
        }
        let snapshot = Arc::new(RuntimeSampleSnapshot { metrics, processes });
        inner.completed_at = Some(Instant::now());
        inner.snapshot = Some(Arc::clone(&snapshot));
        Ok(snapshot)
    }
    pub(crate) fn terminate(&self, pid: u32) -> RuntimeProcessTerminationOutcome {
        let Ok(mut inner) = self.inner.lock() else {
            return RuntimeProcessTerminationOutcome::Failed;
        };
        // Revalidate the target against the live OS, never authorize from a cached projection.
        inner.sampler.terminate(pid)
    }
    #[cfg(test)]
    pub(crate) fn raw_refresh_count(&self) -> usize {
        self.inner.lock().unwrap().raw_refresh_count
    }
    #[cfg(test)]
    pub(crate) fn contains_userland_tasks(&self) -> bool {
        self.inner.lock().unwrap().sampler.contains_userland_tasks()
    }
}
impl Default for RuntimeSampleSource {
    fn default() -> Self {
        Self::new(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL)
    }
}
