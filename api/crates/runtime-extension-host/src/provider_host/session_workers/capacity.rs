#[cfg(target_os = "linux")]
use std::fs;
use std::sync::{Arc, Mutex};

use tokio::sync::Notify;
#[cfg(test)]
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::super::operations::transport_binding_error;
use extension_package_runtime::error::FrameworkResult;

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;
const DEFAULT_WORKER_BUDGET: u64 = 512 * MIB;
const MIN_WORKER_BUDGET: u64 = 256 * MIB;

#[derive(Debug, Clone, Copy)]
struct MemorySnapshot {
    total: u64,
    available: u64,
}

#[derive(Debug)]
struct MemoryBudget {
    reserved: Mutex<u64>,
    #[cfg(test)]
    fixed_snapshot: Option<MemorySnapshot>,
}

#[derive(Debug, Clone)]
enum CapacityMode {
    Memory(Arc<MemoryBudget>),
    #[cfg(test)]
    Fixed(Arc<Semaphore>),
}

#[derive(Debug, Clone)]
pub(crate) struct SessionWorkerCapacity {
    mode: CapacityMode,
    pub(super) changed: Arc<Notify>,
}

#[derive(Debug)]
pub(crate) struct SessionWorkerPermit(PermitMode);

#[derive(Debug)]
enum PermitMode {
    Memory {
        budget: Arc<MemoryBudget>,
        changed: Arc<Notify>,
        bytes: u64,
    },
    #[cfg(test)]
    Fixed { _slot: OwnedSemaphorePermit },
}

impl Drop for SessionWorkerPermit {
    fn drop(&mut self) {
        match &self.0 {
            PermitMode::Memory {
                budget,
                changed,
                bytes,
            } => {
                let mut reserved = budget.reserved.lock().expect("worker memory budget");
                *reserved = reserved.saturating_sub(*bytes);
                changed.notify_waiters();
            }
            #[cfg(test)]
            PermitMode::Fixed { .. } => {}
        }
    }
}

impl Default for SessionWorkerCapacity {
    fn default() -> Self {
        Self {
            mode: CapacityMode::Memory(Arc::new(MemoryBudget {
                reserved: Mutex::new(0),
                #[cfg(test)]
                fixed_snapshot: None,
            })),
            changed: Arc::new(Notify::new()),
        }
    }
}

impl SessionWorkerCapacity {
    pub(super) fn try_acquire(
        &self,
        limit: Option<u64>,
    ) -> FrameworkResult<Option<SessionWorkerPermit>> {
        match &self.mode {
            CapacityMode::Memory(budget) => {
                let bytes = limit
                    .unwrap_or(DEFAULT_WORKER_BUDGET)
                    .max(MIN_WORKER_BUDGET);
                let mut reserved = budget.reserved.lock().expect("worker memory budget");
                let snapshot = memory_snapshot(budget)?;
                // MemAvailable already accounts for actual worker pages. Reserving
                // their full configured ceilings as well is conservative and
                // protects a burst whose children have not reached peak RSS yet.
                let reserve_floor = (snapshot.total / 4).min(2 * GIB);
                if snapshot.available.saturating_sub(*reserved)
                    < reserve_floor.saturating_add(bytes)
                {
                    return Ok(None);
                }
                *reserved = reserved.saturating_add(bytes);
                Ok(Some(SessionWorkerPermit(PermitMode::Memory {
                    budget: Arc::clone(budget),
                    changed: Arc::clone(&self.changed),
                    bytes,
                })))
            }
            #[cfg(test)]
            CapacityMode::Fixed(slots) => Ok(slots
                .clone()
                .try_acquire_owned()
                .ok()
                .map(|slot| SessionWorkerPermit(PermitMode::Fixed { _slot: slot }))),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(capacity: usize) -> Self {
        Self {
            mode: CapacityMode::Fixed(Arc::new(Semaphore::new(capacity))),
            changed: Arc::new(Notify::new()),
        }
    }

    #[cfg(test)]
    fn for_test_memory(total: u64, available: u64) -> Self {
        Self {
            mode: CapacityMode::Memory(Arc::new(MemoryBudget {
                reserved: Mutex::new(0),
                fixed_snapshot: Some(MemorySnapshot { total, available }),
            })),
            changed: Arc::new(Notify::new()),
        }
    }
}

fn memory_snapshot(_budget: &MemoryBudget) -> FrameworkResult<MemorySnapshot> {
    #[cfg(test)]
    if let Some(snapshot) = _budget.fixed_snapshot {
        return Ok(snapshot);
    }
    #[cfg(target_os = "linux")]
    return linux_memory_snapshot();
    #[cfg(not(target_os = "linux"))]
    {
        let mut system = sysinfo::System::new();
        system.refresh_memory();
        Ok(MemorySnapshot {
            total: system.total_memory(),
            available: system.available_memory(),
        })
    }
}

#[cfg(target_os = "linux")]
fn linux_memory_snapshot() -> FrameworkResult<MemorySnapshot> {
    let meminfo = fs::read_to_string("/proc/meminfo")
        .map_err(|_| transport_binding_error("worker memory availability is unknown"))?;
    let kib = |name: &str| -> Option<u64> {
        meminfo.lines().find_map(|line| {
            let value = line.strip_prefix(name)?.split_whitespace().next()?;
            value.parse::<u64>().ok()?.checked_mul(1024)
        })
    };
    let mut snapshot = MemorySnapshot {
        total: kib("MemTotal:")
            .ok_or_else(|| transport_binding_error("worker memory total is unknown"))?,
        available: kib("MemAvailable:")
            .ok_or_else(|| transport_binding_error("worker memory availability is unknown"))?,
    };
    if let Ok(maximum) = fs::read_to_string("/sys/fs/cgroup/memory.max") {
        if let Ok(maximum) = maximum.trim().parse::<u64>() {
            let current = fs::read_to_string("/sys/fs/cgroup/memory.current")
                .ok()
                .and_then(|value| value.trim().parse::<u64>().ok())
                .ok_or_else(|| transport_binding_error("cgroup memory usage is unknown"))?;
            snapshot.total = snapshot.total.min(maximum);
            snapshot.available = snapshot.available.min(maximum.saturating_sub(current));
        }
    }
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abundant_memory_admits_beyond_sixty_four_without_a_fixed_slot_ceiling() {
        let capacity = SessionWorkerCapacity::for_test_memory(64 * GIB, 64 * GIB);
        let permits: Vec<_> = (0..65)
            .map(|_| {
                capacity
                    .try_acquire(Some(MIN_WORKER_BUDGET))
                    .unwrap()
                    .unwrap()
            })
            .collect();
        assert_eq!(permits.len(), 65);
    }

    #[test]
    fn memory_reservation_blocks_and_returns_after_confirmed_release() {
        let capacity = SessionWorkerCapacity::for_test_memory(2 * GIB, 900 * MIB);
        let first = capacity.try_acquire(Some(256 * MIB)).unwrap().unwrap();
        assert!(capacity.try_acquire(Some(256 * MIB)).unwrap().is_none());
        drop(first);
        assert!(capacity.try_acquire(Some(256 * MIB)).unwrap().is_some());
    }
}
