//! Shared on-demand metrics with process metadata and rows initialized on demand.
use crate::{
    RuntimeMetricSampler, RuntimeMetricsSnapshot, RuntimeProcessSnapshot,
    RuntimeProcessTerminationOutcome,
};
use anyhow::{anyhow, Result};
use std::{
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct RuntimeSampleSnapshot {
    pub metrics: RuntimeMetricsSnapshot,
    processes: OnceLock<Arc<RuntimeProcessSnapshot>>,
}
#[derive(Debug)]
struct SampleSourceInner {
    sampler: RuntimeMetricSampler,
    completed_at: Option<Instant>,
    snapshot: Option<Arc<RuntimeSampleSnapshot>>,
    #[cfg(test)]
    // Broad PID enumerations only; user/network/disk initialization and live guard reads are separate.
    raw_refresh_count: usize,
    #[cfg(test)]
    metric_refresh_count: usize,
    #[cfg(test)]
    process_projection_count: usize,
}
/// Composition Root owns this source; it has no timer or persistent writes.
/// The mutex combines misses; metrics stay immutable and process rows are initialized once.
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
                #[cfg(test)]
                metric_refresh_count: 0,
                #[cfg(test)]
                process_projection_count: 0,
            }),
        }
    }
    pub fn collect(&self) -> Result<Arc<RuntimeSampleSnapshot>> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow!("runtime sample source lock poisoned"))?;
        Ok(self.collect_locked(&mut inner))
    }
    fn collect_locked(&self, inner: &mut SampleSourceInner) -> Arc<RuntimeSampleSnapshot> {
        if inner
            .completed_at
            .is_some_and(|at| at.elapsed() < self.freshness)
        {
            if let Some(snapshot) = &inner.snapshot {
                return Arc::clone(snapshot);
            }
        }
        let metrics = inner.sampler.collect();
        #[cfg(test)]
        {
            inner.raw_refresh_count += 1;
            inner.metric_refresh_count += 1;
        }
        let snapshot = Arc::new(RuntimeSampleSnapshot {
            metrics,
            processes: OnceLock::new(),
        });
        inner.completed_at = Some(Instant::now());
        inner.snapshot = Some(Arc::clone(&snapshot));
        snapshot
    }
    fn project_processes_locked(
        &self,
        inner: &mut SampleSourceInner,
        snapshot: &RuntimeSampleSnapshot,
    ) -> Arc<RuntimeProcessSnapshot> {
        Arc::clone(snapshot.processes.get_or_init(|| {
            let metadata_refreshed = inner.sampler.ensure_process_metadata();
            #[cfg(test)]
            {
                inner.raw_refresh_count += usize::from(metadata_refreshed);
                inner.process_projection_count += 1;
            }
            #[cfg(not(test))]
            let _ = metadata_refreshed;
            Arc::new(inner.sampler.process_snapshot())
        }))
    }
    pub(crate) fn collect_processes(&self) -> Result<Arc<RuntimeProcessSnapshot>> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow!("runtime sample source lock poisoned"))?;
        inner.sampler.request_process_metadata();
        let snapshot = self.collect_locked(&mut inner);
        Ok(self.project_processes_locked(&mut inner, &snapshot))
    }
    pub(crate) fn terminate(&self, pid: u32) -> RuntimeProcessTerminationOutcome {
        let Ok(mut inner) = self.inner.lock() else {
            return RuntimeProcessTerminationOutcome::Failed;
        };
        // Freeze the optional projection before live validation mutates the
        // sampler. Authorization still reads the live target, not this cache.
        inner.sampler.request_process_metadata();
        let snapshot = self.collect_locked(&mut inner);
        self.project_processes_locked(&mut inner, &snapshot);
        inner.sampler.terminate(pid)
    }
    #[cfg(test)]
    pub(crate) fn raw_refresh_count(&self) -> usize {
        self.inner.lock().unwrap().raw_refresh_count
    }
    #[cfg(test)]
    pub(crate) fn metric_refresh_count(&self) -> usize {
        self.inner.lock().unwrap().metric_refresh_count
    }
    #[cfg(test)]
    pub(crate) fn process_projection_count(&self) -> usize {
        self.inner.lock().unwrap().process_projection_count
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
