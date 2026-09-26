#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;
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
pub(super) struct MemorySnapshot {
    pub(super) total: u64,
    pub(super) available: u64,
}

#[derive(Debug)]
struct MemoryBudget {
    reservations: Mutex<Reservations>,
    #[cfg(test)]
    fixed_snapshot: Option<Mutex<MemorySnapshot>>,
}

#[derive(Debug, Default)]
struct Reservations {
    committed: u64,
    // Captured before the first managed session worker starts. Worker pages
    // subsequently reduce MemAvailable but do not reduce this promise twice.
    pool_limit: Option<u64>,
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
                let mut reservations = budget.reservations.lock().expect("worker memory budget");
                reservations.committed = reservations.committed.saturating_sub(*bytes);
                if reservations.committed == 0 {
                    reservations.pool_limit = None;
                }
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
                reservations: Mutex::new(Reservations::default()),
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
                let mut reservations = budget.reservations.lock().expect("worker memory budget");
                let snapshot = memory_snapshot(budget)?;
                let reserve_floor = (snapshot.total / 4).min(2 * GIB);
                let pool_limit = reservations
                    .pool_limit
                    .unwrap_or_else(|| snapshot.available.saturating_sub(reserve_floor));
                let next_commitment = reservations.committed.saturating_add(bytes);
                // The pool keeps every live worker's full future growth promise.
                // The independent pressure gate responds to memory consumed by
                // this Host and other processes since that pool was established.
                // External growth can still erode old commitments; this is an
                // admission policy, not a hard cgroup or process memory limit.
                if next_commitment > pool_limit
                    || snapshot.available < reserve_floor.saturating_add(bytes)
                {
                    tracing::debug!(
                        available_bytes = snapshot.available,
                        committed_bytes = reservations.committed,
                        pool_limit_bytes = pool_limit,
                        reserve_floor_bytes = reserve_floor,
                        requested_bytes = bytes,
                        "session worker memory admission waiting"
                    );
                    return Ok(None);
                }
                reservations.pool_limit = Some(pool_limit);
                reservations.committed = next_commitment;
                tracing::debug!(
                    committed_bytes = next_commitment,
                    pool_limit_bytes = pool_limit,
                    requested_bytes = bytes,
                    "session worker memory admission granted"
                );
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
                reservations: Mutex::new(Reservations::default()),
                fixed_snapshot: Some(Mutex::new(MemorySnapshot { total, available })),
            })),
            changed: Arc::new(Notify::new()),
        }
    }

    #[cfg(test)]
    fn set_test_available(&self, available: u64) {
        if let CapacityMode::Memory(budget) = &self.mode {
            budget
                .fixed_snapshot
                .as_ref()
                .expect("fixed memory snapshot")
                .lock()
                .expect("fixed memory snapshot")
                .available = available;
        }
    }
}

fn memory_snapshot(_budget: &MemoryBudget) -> FrameworkResult<MemorySnapshot> {
    #[cfg(test)]
    if let Some(snapshot) = &_budget.fixed_snapshot {
        return Ok(*snapshot.lock().expect("fixed memory snapshot"));
    }
    system_memory_snapshot()
}

pub(super) fn system_memory_snapshot() -> FrameworkResult<MemorySnapshot> {
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
    let membership = fs::read_to_string("/proc/self/cgroup")
        .map_err(|_| transport_binding_error("worker cgroup membership is unknown"))?;
    clamp_to_cgroup_limits(&mut snapshot, Path::new("/sys/fs/cgroup"), &membership)?;
    Ok(snapshot)
}

#[cfg(target_os = "linux")]
fn clamp_to_cgroup_limits(
    snapshot: &mut MemorySnapshot,
    root: &Path,
    membership: &str,
) -> FrameworkResult<()> {
    let mut path = root.to_path_buf();
    if let Some(relative) = membership.lines().find_map(|line| line.strip_prefix("0::")) {
        for component in relative
            .split('/')
            .filter(|component| !component.is_empty())
        {
            if component == "." || component == ".." {
                return Err(transport_binding_error("cgroup membership is invalid"));
            }
            path.push(component);
        }
    } else if root.join("cgroup.controllers").exists() {
        return Err(transport_binding_error(
            "worker cgroup membership is unknown",
        ));
    }
    loop {
        if let Ok(maximum) = fs::read_to_string(path.join("memory.max")) {
            if let Ok(maximum) = maximum.trim().parse::<u64>() {
                let current = fs::read_to_string(path.join("memory.current"))
                    .ok()
                    .and_then(|value| value.trim().parse::<u64>().ok())
                    .ok_or_else(|| transport_binding_error("cgroup memory usage is unknown"))?;
                snapshot.total = snapshot.total.min(maximum);
                snapshot.available = snapshot.available.min(maximum.saturating_sub(current));
            }
        }
        if path == root {
            break;
        }
        path.pop();
    }
    Ok(())
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

    #[test]
    fn realized_worker_pages_do_not_consume_their_peak_promise_twice() {
        let capacity = SessionWorkerCapacity::for_test_memory(2 * GIB, 2 * GIB);
        let first = capacity.try_acquire(Some(512 * MIB)).unwrap().unwrap();
        capacity.set_test_available(1536 * MIB);
        let second = capacity.try_acquire(Some(512 * MIB)).unwrap().unwrap();
        capacity.set_test_available(1024 * MIB);
        let third = capacity.try_acquire(Some(512 * MIB)).unwrap().unwrap();
        assert!(capacity.try_acquire(Some(512 * MIB)).unwrap().is_none());
        drop((first, second, third));
        capacity.set_test_available(768 * MIB);
        assert!(capacity.try_acquire(Some(512 * MIB)).unwrap().is_none());
        capacity.set_test_available(2 * GIB);
        assert!(capacity.try_acquire(Some(512 * MIB)).unwrap().is_some());
    }

    #[test]
    fn new_external_pressure_blocks_worker_even_with_unused_pool_budget() {
        let capacity = SessionWorkerCapacity::for_test_memory(4 * GIB, 4 * GIB);
        let first = capacity.try_acquire(Some(512 * MIB)).unwrap().unwrap();
        capacity.set_test_available(1024 * MIB);
        assert!(capacity.try_acquire(Some(512 * MIB)).unwrap().is_none());
        drop(first);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn nested_cgroup_uses_its_ancestor_memory_limit() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("worker-capacity-{}-{nonce}", std::process::id()));
        let parent = root.join("parent");
        let child = parent.join("child");
        fs::create_dir_all(&child).unwrap();
        fs::write(root.join("memory.max"), "6442450944").unwrap();
        fs::write(root.join("memory.current"), "2147483648").unwrap();
        fs::write(parent.join("memory.max"), "2147483648").unwrap();
        fs::write(parent.join("memory.current"), "1610612736").unwrap();
        fs::write(child.join("memory.max"), "max").unwrap();
        let mut snapshot = MemorySnapshot {
            total: 8 * GIB,
            available: 7 * GIB,
        };
        let result = clamp_to_cgroup_limits(&mut snapshot, &root, "0::/parent/child\n");
        fs::remove_dir_all(&root).unwrap();
        result.unwrap();
        assert_eq!(snapshot.total, 2 * GIB);
        assert_eq!(snapshot.available, 512 * MIB);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cgroup_v2_without_membership_fails_closed() {
        let root =
            std::env::temp_dir().join(format!("worker-cgroup-membership-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("cgroup.controllers"), "memory").unwrap();
        let mut snapshot = MemorySnapshot {
            total: 8 * GIB,
            available: 7 * GIB,
        };
        let result = clamp_to_cgroup_limits(&mut snapshot, &root, "1:memory:/other\n");
        fs::remove_dir_all(&root).unwrap();
        assert!(result.is_err());
    }
}
